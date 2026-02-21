//! Audio-Payload Verschluesselung
//!
//! Verschluesselt Opus-Audio-Pakete mit dem Channel-Gruppen-Schluessel.
//!
//! ## Format
//! ```text
//! [nonce(12)] [aad_len(2)] [aad] [ciphertext + auth_tag(16)]
//! ```
//!
//! ## Nonce-Aufbau
//! ```text
//! [epoch(4)] [sequence(4)] [random(4)]
//! ```
//!
//! ## AAD (Authenticated Additional Data)
//! ```text
//! [ssrc(4)] [epoch(4)]
//! ```

use aes_gcm::{
    aead::{Aead, KeyInit, Payload},
    Aes256Gcm, Key, Nonce as AesNonce,
};
use chacha20poly1305::{ChaCha20Poly1305, Key as ChaChaKey, Nonce as ChaChaNonce};
use rand::rngs::OsRng;
use rand::RngCore;

use crate::error::{CryptoError, CryptoResult};
use crate::types::{EncryptedPayload, GroupKey, GroupKeyAlgorithm, Nonce};

/// Verschluesselt einen Audio-Payload mit dem Channel-Gruppenschluessel
///
/// # Parameter
/// - `plaintext`: Rohe Opus-Audio-Daten
/// - `key`: Aktiver Gruppen-Schluessel
/// - `ssrc`: Synchronization Source (Identifiziert den Sender)
/// - `seq`: Paket-Sequenz-Nummer
pub fn encrypt_audio(
    plaintext: &[u8],
    key: &GroupKey,
    ssrc: u32,
    seq: u32,
) -> CryptoResult<EncryptedPayload> {
    // Nonce: epoch + seq + random(4)
    let mut random_bytes = [0u8; 4];
    OsRng.fill_bytes(&mut random_bytes);
    let nonce = Nonce::aus_epoch_und_seq(key.epoch, seq, random_bytes);

    // AAD: ssrc(4) + epoch(4)
    let mut aad = Vec::with_capacity(8);
    aad.extend_from_slice(&ssrc.to_be_bytes());
    aad.extend_from_slice(&key.epoch.to_be_bytes());

    let ciphertext = match key.algorithm {
        GroupKeyAlgorithm::Aes256Gcm => {
            encrypt_aes256gcm(plaintext, key.key_bytes.as_bytes(), nonce.as_bytes(), &aad)?
        }
        GroupKeyAlgorithm::ChaCha20Poly1305 => {
            encrypt_chacha20(plaintext, key.key_bytes.as_bytes(), nonce.as_bytes(), &aad)?
        }
    };

    Ok(EncryptedPayload { nonce, ciphertext, aad })
}

fn encrypt_aes256gcm(
    plaintext: &[u8],
    key_bytes: &[u8],
    nonce_bytes: &[u8; 12],
    aad: &[u8],
) -> CryptoResult<Vec<u8>> {
    if key_bytes.len() != 32 {
        return Err(CryptoError::UngueltigeSchluesselLaenge {
            erwartet: 32,
            erhalten: key_bytes.len(),
        });
    }

    let key = Key::<Aes256Gcm>::from_slice(key_bytes);
    let cipher = Aes256Gcm::new(key);
    let nonce = AesNonce::from_slice(nonce_bytes);

    cipher
        .encrypt(nonce, Payload { msg: plaintext, aad })
        .map_err(|e| CryptoError::Verschluesselung(e.to_string()))
}

fn encrypt_chacha20(
    plaintext: &[u8],
    key_bytes: &[u8],
    nonce_bytes: &[u8; 12],
    aad: &[u8],
) -> CryptoResult<Vec<u8>> {
    if key_bytes.len() != 32 {
        return Err(CryptoError::UngueltigeSchluesselLaenge {
            erwartet: 32,
            erhalten: key_bytes.len(),
        });
    }

    let key = ChaChaKey::from_slice(key_bytes);
    let cipher = ChaCha20Poly1305::new(key);
    let nonce = ChaChaNonce::from_slice(nonce_bytes);

    cipher
        .encrypt(nonce, chacha20poly1305::aead::Payload { msg: plaintext, aad })
        .map_err(|e| CryptoError::Verschluesselung(e.to_string()))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::e2e::group_key::create_group_key;

    #[test]
    fn encrypt_audio_aes256gcm() {
        let key =
            create_group_key("test-ch", 1, 0, GroupKeyAlgorithm::Aes256Gcm).unwrap();
        let plaintext = b"Hallo Opus-Audio-Daten 1234567890";

        let payload = encrypt_audio(plaintext, &key, 42, 1).unwrap();

        // Nonce-Struktur pruefen
        assert_eq!(payload.nonce.epoch(), 0);
        assert_eq!(payload.nonce.seq(), 1);

        // AAD-Struktur pruefen: ssrc(4) + epoch(4)
        assert_eq!(payload.aad.len(), 8);
        let ssrc = u32::from_be_bytes(payload.aad[0..4].try_into().unwrap());
        assert_eq!(ssrc, 42);

        // Ciphertext hat Overhead (16 Bytes Auth-Tag)
        assert!(payload.ciphertext.len() > plaintext.len());
    }

    #[test]
    fn encrypt_audio_chacha20() {
        let key =
            create_group_key("test-ch", 1, 0, GroupKeyAlgorithm::ChaCha20Poly1305).unwrap();
        let plaintext = b"Opus-Frame-Daten";

        let payload = encrypt_audio(plaintext, &key, 99, 7).unwrap();
        assert_eq!(payload.nonce.seq(), 7);
        assert!(payload.ciphertext.len() >= plaintext.len() + 16);
    }

    #[test]
    fn nonce_epoch_und_seq_korrekt() {
        let key =
            create_group_key("ch", 1, 5, GroupKeyAlgorithm::Aes256Gcm).unwrap();
        let payload = encrypt_audio(b"test", &key, 0, 100).unwrap();
        assert_eq!(payload.nonce.epoch(), 5);
        assert_eq!(payload.nonce.seq(), 100);
    }

    #[test]
    fn payload_serialisierung() {
        let key =
            create_group_key("ch", 1, 0, GroupKeyAlgorithm::Aes256Gcm).unwrap();
        let payload = encrypt_audio(b"audio", &key, 1, 2).unwrap();

        let bytes = payload.to_bytes();
        let restored = EncryptedPayload::from_bytes(&bytes).unwrap();

        assert_eq!(restored.nonce.bytes, payload.nonce.bytes);
        assert_eq!(restored.ciphertext, payload.ciphertext);
        assert_eq!(restored.aad, payload.aad);
    }
}
