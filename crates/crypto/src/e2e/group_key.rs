//! Gruppen-Schluessel pro Channel
//!
//! Jeder Channel hat einen symmetrischen Schluessel (AES-256-GCM oder ChaCha20-Poly1305).
//! Bei Join/Leave wird der Schluessel rotiert (neue Epoch).

use rand::rngs::OsRng;
use rand::RngCore;

use crate::error::{CryptoError, CryptoResult};
use crate::types::{GroupKey, GroupKeyAlgorithm, SecretBytes};

/// Erstellt einen neuen Gruppen-Schluessel fuer einen Channel
pub fn create_group_key(
    channel_id: &str,
    key_id: u64,
    epoch: u32,
    algorithm: GroupKeyAlgorithm,
) -> CryptoResult<GroupKey> {
    let mut key_bytes = vec![0u8; 32];
    OsRng.fill_bytes(&mut key_bytes);

    Ok(GroupKey {
        key_id,
        epoch,
        key_bytes: SecretBytes::new(key_bytes),
        algorithm,
        channel_id: channel_id.to_string(),
    })
}

/// Rotiert einen Gruppen-Schluessel (erhoehte Epoch, neue Key-ID)
pub fn rotate_group_key(current: &GroupKey) -> CryptoResult<GroupKey> {
    create_group_key(
        &current.channel_id,
        current.key_id + 1,
        current.epoch + 1,
        current.algorithm,
    )
}

/// Verschluesselt einen Gruppen-Schluessel mit dem oeffentlichen X25519-Schluessel
/// eines Empfaengers (fuer Key Distribution).
///
/// Nutzt ECIES-aehnliches Schema:
/// 1. Ephemeres X25519-Schluessel-Paar generieren
/// 2. DH mit Empfaenger-Public-Key
/// 3. HKDF -> Wrapping Key
/// 4. AES-256-GCM verschluesseln
pub fn wrap_key_for_recipient(
    group_key: &GroupKey,
    recipient_public_key: &[u8; 32],
) -> CryptoResult<Vec<u8>> {
    use crate::e2e::key_exchange::hkdf_derive;
    use aes_gcm::{
        aead::{Aead, KeyInit},
        Aes256Gcm, Key, Nonce,
    };
    use x25519_dalek::{EphemeralSecret, PublicKey as X25519PublicKey};

    // Ephemeres Schluessel-Paar
    let ephemeral_secret = EphemeralSecret::random_from_rng(OsRng);
    let ephemeral_public = X25519PublicKey::from(&ephemeral_secret);

    // DH-Austausch
    let recipient_pk = X25519PublicKey::from(*recipient_public_key);
    let dh_output = ephemeral_secret.diffie_hellman(&recipient_pk);

    // HKDF -> Wrapping Key (32 Bytes)
    let wrapping_key = hkdf_derive(
        dh_output.as_bytes(),
        recipient_public_key,
        b"speakeasy-key-wrap-v1",
        32,
    )?;

    // AES-256-GCM verschluesseln
    let cipher_key = Key::<Aes256Gcm>::from_slice(&wrapping_key);
    let cipher = Aes256Gcm::new(cipher_key);

    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, group_key.key_bytes.as_bytes())
        .map_err(|e| CryptoError::Verschluesselung(e.to_string()))?;

    // Output: [ephemeral_public(32)] + [nonce(12)] + [ciphertext]
    let mut out = Vec::with_capacity(32 + 12 + ciphertext.len());
    out.extend_from_slice(ephemeral_public.as_bytes());
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);

    Ok(out)
}

/// Entschluesselt einen eingewickelten Gruppen-Schluessel mit dem eigenen
/// privaten X25519-Schluessel.
pub fn unwrap_key_for_recipient(
    wrapped: &[u8],
    recipient_private_key: &[u8; 32],
    channel_id: &str,
    key_id: u64,
    epoch: u32,
    algorithm: GroupKeyAlgorithm,
) -> CryptoResult<GroupKey> {
    use crate::e2e::key_exchange::hkdf_derive;
    use aes_gcm::{
        aead::{Aead, KeyInit},
        Aes256Gcm, Key, Nonce,
    };
    use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret};

    if wrapped.len() < 32 + 12 + 16 {
        return Err(CryptoError::UngueltigeDaten(
            "Zu kurzer wrapped key".to_string(),
        ));
    }

    let ephemeral_pub_bytes: [u8; 32] = wrapped[0..32].try_into().unwrap();
    let nonce_bytes: [u8; 12] = wrapped[32..44].try_into().unwrap();
    let ciphertext = &wrapped[44..];

    // DH mit dem empfaengerseitigen privaten Schluessel
    let private_key = StaticSecret::from(*recipient_private_key);
    let ephemeral_pub = X25519PublicKey::from(ephemeral_pub_bytes);
    let dh_output = private_key.diffie_hellman(&ephemeral_pub);

    // HKDF -> Wrapping Key
    let recipient_pub = X25519PublicKey::from(&private_key);
    let wrapping_key = hkdf_derive(
        dh_output.as_bytes(),
        recipient_pub.as_bytes(),
        b"speakeasy-key-wrap-v1",
        32,
    )?;

    // AES-256-GCM entschluesseln
    let cipher_key = Key::<Aes256Gcm>::from_slice(&wrapping_key);
    let cipher = Aes256Gcm::new(cipher_key);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| CryptoError::Entschluesselung(e.to_string()))?;

    Ok(GroupKey {
        key_id,
        epoch,
        key_bytes: SecretBytes::new(plaintext),
        algorithm,
        channel_id: channel_id.to_string(),
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_key_erstellen() {
        let key = create_group_key("channel-1", 1, 0, GroupKeyAlgorithm::Aes256Gcm).unwrap();
        assert_eq!(key.key_bytes.len(), 32);
        assert_eq!(key.epoch, 0);
        assert_eq!(key.key_id, 1);
        assert_eq!(key.channel_id, "channel-1");
    }

    #[test]
    fn group_key_rotation_erhoht_epoch() {
        let key1 = create_group_key("channel-1", 1, 0, GroupKeyAlgorithm::Aes256Gcm).unwrap();
        let key2 = rotate_group_key(&key1).unwrap();

        assert_eq!(key2.epoch, 1);
        assert_eq!(key2.key_id, 2);
        // Neuer Schluessel muss verschieden sein
        assert_ne!(key1.key_bytes.as_bytes(), key2.key_bytes.as_bytes());
    }

    #[test]
    fn group_key_wrap_und_unwrap_roundtrip() {
        use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret};

        // Empfaenger-Schluessel-Paar (statisch fuer Test)
        let mut priv_bytes = [0u8; 32];
        OsRng.fill_bytes(&mut priv_bytes);
        let private_key = StaticSecret::from(priv_bytes);
        let public_key = X25519PublicKey::from(&private_key);

        let original_key =
            create_group_key("test-channel", 5, 2, GroupKeyAlgorithm::Aes256Gcm).unwrap();
        let original_bytes = original_key.key_bytes.as_bytes().to_vec();

        // Einwickeln
        let wrapped = wrap_key_for_recipient(&original_key, public_key.as_bytes()).unwrap();

        // Auswickeln
        let unwrapped = unwrap_key_for_recipient(
            &wrapped,
            &priv_bytes,
            "test-channel",
            5,
            2,
            GroupKeyAlgorithm::Aes256Gcm,
        )
        .unwrap();

        assert_eq!(unwrapped.key_bytes.as_bytes(), original_bytes.as_slice());
        assert_eq!(unwrapped.epoch, 2);
        assert_eq!(unwrapped.key_id, 5);
    }

    #[test]
    fn falscher_private_key_schlaegt_fehl() {
        use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret};

        let mut priv_bytes = [0u8; 32];
        OsRng.fill_bytes(&mut priv_bytes);
        let private_key = StaticSecret::from(priv_bytes);
        let public_key = X25519PublicKey::from(&private_key);

        let key = create_group_key("ch", 1, 0, GroupKeyAlgorithm::Aes256Gcm).unwrap();
        let wrapped = wrap_key_for_recipient(&key, public_key.as_bytes()).unwrap();

        // Falscher privater Schluessel
        let mut wrong_priv = [0u8; 32];
        OsRng.fill_bytes(&mut wrong_priv);

        let result = unwrap_key_for_recipient(
            &wrapped,
            &wrong_priv,
            "ch",
            1,
            0,
            GroupKeyAlgorithm::Aes256Gcm,
        );
        assert!(result.is_err());
    }

    #[test]
    fn zu_kurzer_wrapped_key_schlaegt_fehl() {
        let mut priv_bytes = [0u8; 32];
        OsRng.fill_bytes(&mut priv_bytes);

        let result = unwrap_key_for_recipient(
            &[0u8; 10],
            &priv_bytes,
            "ch",
            1,
            0,
            GroupKeyAlgorithm::Aes256Gcm,
        );
        assert!(result.is_err());
    }
}
