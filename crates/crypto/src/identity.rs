//! Langzeit-Identitaetsschluessel (Ed25519)
//!
//! Jeder Benutzer erhaelt beim Registrieren ein Ed25519-Schluessel-Paar.
//! Der oeffentliche Schluessel wird auf dem Server gespeichert, der
//! private Schluessel verbleibt beim Client.

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};

use crate::error::CryptoResult;

/// Langzeit-Identitaet eines Benutzers (Ed25519)
pub struct Identity {
    signing_key: SigningKey,
}

/// Oeffentliche Identitaet (nur Verifying Key)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicIdentity {
    /// Ed25519 Verifying Key (32 Bytes, Base64-kodiert)
    pub public_key_bytes: [u8; 32],
}

impl Identity {
    /// Generiert ein neues Ed25519-Schluessel-Paar
    pub fn generate() -> Self {
        let signing_key = SigningKey::generate(&mut OsRng);
        Self { signing_key }
    }

    /// Erstellt eine Identity aus einem privaten Schluessel (32 Bytes)
    pub fn from_bytes(bytes: &[u8; 32]) -> CryptoResult<Self> {
        let signing_key = SigningKey::from_bytes(bytes);
        Ok(Self { signing_key })
    }

    /// Gibt den privaten Schluessel als Bytes zurueck (fuer Persistenz)
    pub fn private_key_bytes(&self) -> [u8; 32] {
        self.signing_key.to_bytes()
    }

    /// Gibt den oeffentlichen Schluessel als Bytes zurueck
    pub fn public_key_bytes(&self) -> [u8; 32] {
        self.signing_key.verifying_key().to_bytes()
    }

    /// Gibt die oeffentliche Identitaet zurueck
    pub fn public_identity(&self) -> PublicIdentity {
        PublicIdentity {
            public_key_bytes: self.public_key_bytes(),
        }
    }

    /// Signiert Daten mit dem privaten Schluessel
    pub fn sign(&self, data: &[u8]) -> CryptoResult<Vec<u8>> {
        let signature = self.signing_key.sign(data);
        Ok(signature.to_bytes().to_vec())
    }

    /// Verifiziert eine Signatur mit einem oeffentlichen Schluessel
    pub fn verify(data: &[u8], signature_bytes: &[u8], public_key_bytes: &[u8; 32]) -> bool {
        let Ok(verifying_key) = VerifyingKey::from_bytes(public_key_bytes) else {
            return false;
        };
        let Ok(sig_array) = signature_bytes.try_into() else {
            return false;
        };
        let signature = Signature::from_bytes(sig_array);
        verifying_key.verify(data, &signature).is_ok()
    }
}

impl std::fmt::Debug for Identity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Identity {{ public_key: [Ed25519 VerifyingKey] }}")
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_generieren() {
        let identity = Identity::generate();
        let pub_bytes = identity.public_key_bytes();
        assert_eq!(pub_bytes.len(), 32);
    }

    #[test]
    fn identity_signieren_und_verifizieren() {
        let identity = Identity::generate();
        let data = b"Hallo, Speakeasy!";

        let signature = identity.sign(data).unwrap();
        assert_eq!(signature.len(), 64);

        let pub_key = identity.public_key_bytes();
        assert!(Identity::verify(data, &signature, &pub_key));
    }

    #[test]
    fn falsche_signatur_wird_abgelehnt() {
        let identity = Identity::generate();
        let data = b"Hallo, Speakeasy!";

        let mut signature = identity.sign(data).unwrap();
        // Signatur manipulieren
        signature[0] ^= 0xFF;

        let pub_key = identity.public_key_bytes();
        assert!(!Identity::verify(data, &signature, &pub_key));
    }

    #[test]
    fn falsche_daten_werden_abgelehnt() {
        let identity = Identity::generate();
        let data = b"Originaltext";
        let signature = identity.sign(data).unwrap();

        let pub_key = identity.public_key_bytes();
        assert!(!Identity::verify(b"Geaenderter Text", &signature, &pub_key));
    }

    #[test]
    fn identity_from_bytes_roundtrip() {
        let identity = Identity::generate();
        let priv_bytes = identity.private_key_bytes();
        let pub_bytes = identity.public_key_bytes();

        let restored = Identity::from_bytes(&priv_bytes).unwrap();
        assert_eq!(restored.public_key_bytes(), pub_bytes);
    }

    #[test]
    fn public_identity_serialisierbar() {
        let identity = Identity::generate();
        let pub_id = identity.public_identity();
        let json = serde_json::to_string(&pub_id).unwrap();
        let decoded: PublicIdentity = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.public_key_bytes, pub_id.public_key_bytes);
    }

    #[test]
    fn verschiedene_keys_ablehnen() {
        let id1 = Identity::generate();
        let id2 = Identity::generate();
        let data = b"Testdaten";

        let sig = id1.sign(data).unwrap();
        // Mit anderem oeffentlichen Key verifizieren - muss fehlschlagen
        let pub_key2 = id2.public_key_bytes();
        assert!(!Identity::verify(data, &sig, &pub_key2));
    }
}
