//! X25519 Diffie-Hellman Key Exchange
//!
//! Implementiert den 4-stufigen Handshake:
//! 1. ClientHello  - Oeffentlicher Schluessel + unterstuetzte Algorithmen
//! 2. ServerHello  - Server's Public Key + gewaehlter Algorithmus
//! 3. KeyExchange  - Client sendet seinen Public Key
//! 4. KeyConfirm   - Handshake abgeschlossen
//!
//! Nach dem Handshake wird HKDF zur Key-Ableitung aus dem Shared Secret genutzt.

use hkdf::Hkdf;
use rand::rngs::OsRng;
use sha2::Sha256;
use x25519_dalek::{EphemeralSecret, PublicKey as X25519PublicKey};

use crate::error::{CryptoError, CryptoResult};
use crate::types::SecretBytes;

/// Ergebnis eines erfolgreichen Key-Exchange
#[derive(Debug)]
pub struct SharedSecret {
    /// Abgeleiteter Session-Schluessel (32 Bytes)
    pub session_key: SecretBytes,
    /// Abgeleiteter Authentifizierungs-Schluessel (32 Bytes)
    pub auth_key: SecretBytes,
}

/// Client-seitige Key-Exchange-Instanz
pub struct KeyExchangeClient {
    ephemeral_secret: Option<EphemeralSecret>,
    pub public_key: [u8; 32],
}

impl KeyExchangeClient {
    /// Erstellt eine neue Client-Instanz mit frischen ephemeren Schluesseln
    pub fn new() -> Self {
        let secret = EphemeralSecret::random_from_rng(OsRng);
        let public_key = X25519PublicKey::from(&secret);
        Self {
            ephemeral_secret: Some(secret),
            public_key: public_key.to_bytes(),
        }
    }

    /// Fuehrt den DH-Austausch durch und leitet Session Keys ab
    ///
    /// # Parameter
    /// - `server_public_key`: 32-Byte oeffentlicher Schluessel des Servers
    /// - `client_random`: 32-Byte Zufallswert des Clients
    /// - `server_random`: 32-Byte Zufallswert des Servers
    pub fn compute_shared_secret(
        &mut self,
        server_public_key: &[u8; 32],
        client_random: &[u8; 32],
        server_random: &[u8; 32],
    ) -> CryptoResult<SharedSecret> {
        let secret = self
            .ephemeral_secret
            .take()
            .ok_or_else(|| CryptoError::KeyExchange("Secret bereits verwendet".to_string()))?;

        let server_pk = X25519PublicKey::from(*server_public_key);
        let dh_output = secret.diffie_hellman(&server_pk);

        derive_keys(dh_output.as_bytes(), client_random, server_random)
    }
}

impl Default for KeyExchangeClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Server-seitige Key-Exchange-Instanz
pub struct KeyExchangeServer {
    ephemeral_secret: Option<EphemeralSecret>,
    pub public_key: [u8; 32],
}

impl KeyExchangeServer {
    /// Erstellt eine neue Server-Instanz mit frischen ephemeren Schluesseln
    pub fn new() -> Self {
        let secret = EphemeralSecret::random_from_rng(OsRng);
        let public_key = X25519PublicKey::from(&secret);
        Self {
            ephemeral_secret: Some(secret),
            public_key: public_key.to_bytes(),
        }
    }

    /// Fuehrt den DH-Austausch durch und leitet Session Keys ab
    pub fn compute_shared_secret(
        &mut self,
        client_public_key: &[u8; 32],
        client_random: &[u8; 32],
        server_random: &[u8; 32],
    ) -> CryptoResult<SharedSecret> {
        let secret = self
            .ephemeral_secret
            .take()
            .ok_or_else(|| CryptoError::KeyExchange("Secret bereits verwendet".to_string()))?;

        let client_pk = X25519PublicKey::from(*client_public_key);
        let dh_output = secret.diffie_hellman(&client_pk);

        derive_keys(dh_output.as_bytes(), client_random, server_random)
    }
}

impl Default for KeyExchangeServer {
    fn default() -> Self {
        Self::new()
    }
}

/// Leitet Session- und Auth-Schluessel via HKDF-SHA256 ab
fn derive_keys(
    dh_secret: &[u8],
    client_random: &[u8; 32],
    server_random: &[u8; 32],
) -> CryptoResult<SharedSecret> {
    // IKM = DH-Output
    // Salt = client_random || server_random
    let mut salt = [0u8; 64];
    salt[..32].copy_from_slice(client_random);
    salt[32..].copy_from_slice(server_random);

    let hk = Hkdf::<Sha256>::new(Some(&salt), dh_secret);

    // Session Key ableiten
    let mut session_key = vec![0u8; 32];
    hk.expand(b"speakeasy-session-key-v1", &mut session_key)
        .map_err(|e| CryptoError::KeyDerivation(e.to_string()))?;

    // Auth Key ableiten
    let mut auth_key = vec![0u8; 32];
    hk.expand(b"speakeasy-auth-key-v1", &mut auth_key)
        .map_err(|e| CryptoError::KeyDerivation(e.to_string()))?;

    Ok(SharedSecret {
        session_key: SecretBytes::new(session_key),
        auth_key: SecretBytes::new(auth_key),
    })
}

/// HKDF-basierte Key Derivation (allgemein verwendbar)
pub fn hkdf_derive(ikm: &[u8], salt: &[u8], info: &[u8], len: usize) -> CryptoResult<Vec<u8>> {
    let hk = Hkdf::<Sha256>::new(Some(salt), ikm);
    let mut okm = vec![0u8; len];
    hk.expand(info, &mut okm)
        .map_err(|e| CryptoError::KeyDerivation(e.to_string()))?;
    Ok(okm)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rand::RngCore;

    fn random_32() -> [u8; 32] {
        let mut buf = [0u8; 32];
        OsRng.fill_bytes(&mut buf);
        buf
    }

    #[test]
    fn key_exchange_erzeugt_gleiche_session_keys() {
        let mut client = KeyExchangeClient::new();
        let mut server = KeyExchangeServer::new();

        let client_random = random_32();
        let server_random = random_32();

        let client_pub = client.public_key;
        let server_pub = server.public_key;

        let client_secret = client
            .compute_shared_secret(&server_pub, &client_random, &server_random)
            .unwrap();

        let server_secret = server
            .compute_shared_secret(&client_pub, &client_random, &server_random)
            .unwrap();

        // Beide Seiten muessen identische Schluessel ableiten
        assert_eq!(
            client_secret.session_key.as_bytes(),
            server_secret.session_key.as_bytes()
        );
        assert_eq!(
            client_secret.auth_key.as_bytes(),
            server_secret.auth_key.as_bytes()
        );
    }

    #[test]
    fn verschiedene_randoms_erzeugen_verschiedene_keys() {
        let mut client1 = KeyExchangeClient::new();
        let mut server1 = KeyExchangeServer::new();

        let client_random1 = random_32();
        let server_random1 = random_32();
        let client_random2 = random_32();
        let server_random2 = random_32();

        let c1_pub = client1.public_key;
        let s1_pub = server1.public_key;

        let secret1 = client1
            .compute_shared_secret(&s1_pub, &client_random1, &server_random1)
            .unwrap();

        let mut client2 = KeyExchangeClient::new();
        let mut server2 = KeyExchangeServer::new();
        let c2_pub = client2.public_key;
        let s2_pub = server2.public_key;

        let secret2 = client2
            .compute_shared_secret(&s2_pub, &client_random2, &server_random2)
            .unwrap();

        // Verschiedene Session Keys
        assert_ne!(
            secret1.session_key.as_bytes(),
            secret2.session_key.as_bytes()
        );

        // server1 und server2 auch ableiten (damit kein unused-Warning)
        let _ = server1.compute_shared_secret(&c1_pub, &client_random1, &server_random1);
        let _ = server2.compute_shared_secret(&c2_pub, &client_random2, &server_random2);
    }

    #[test]
    fn secret_kann_nur_einmal_verwendet_werden() {
        let mut client = KeyExchangeClient::new();
        let server_pub = [0u8; 32];
        let r = random_32();

        let _ = client.compute_shared_secret(&server_pub, &r, &r);
        let result = client.compute_shared_secret(&server_pub, &r, &r);
        assert!(result.is_err());
    }

    #[test]
    fn hkdf_derive_deterministisch() {
        let ikm = b"test-input-key-material";
        let salt = b"test-salt";
        let info = b"test-info";

        let key1 = hkdf_derive(ikm, salt, info, 32).unwrap();
        let key2 = hkdf_derive(ikm, salt, info, 32).unwrap();
        assert_eq!(key1, key2);
        assert_eq!(key1.len(), 32);
    }

    #[test]
    fn hkdf_verschiedene_infos_geben_verschiedene_keys() {
        let ikm = b"gleicher-ikm";
        let salt = b"gleicher-salt";

        let key1 = hkdf_derive(ikm, salt, b"info-1", 32).unwrap();
        let key2 = hkdf_derive(ikm, salt, b"info-2", 32).unwrap();
        assert_ne!(key1, key2);
    }
}
