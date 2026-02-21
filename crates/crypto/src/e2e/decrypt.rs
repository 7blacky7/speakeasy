//! Audio-Payload Entschluesselung
//!
//! Entschluesselt Opus-Audio-Pakete mit dem Channel-Gruppen-Schluessel.
//! Verifiziert dabei den Auth-Tag und die AAD (SSRC + Epoch).

use aes_gcm::{
    aead::{Aead, KeyInit, Payload},
    Aes256Gcm, Key, Nonce as AesNonce,
};
use chacha20poly1305::{ChaCha20Poly1305, Key as ChaChaKey, Nonce as ChaChaNonce};

use crate::error::{CryptoError, CryptoResult};
use crate::types::{EncryptedPayload, GroupKey, GroupKeyAlgorithm};

/// Entschluesselt einen Audio-Payload mit dem Channel-Gruppenschluessel
///
/// Verifiziert automatisch:
/// - Auth-Tag (AEAD-Integritaet)
/// - AAD (SSRC + Epoch muessen uebereinstimmen)
pub fn decrypt_audio(payload: &EncryptedPayload, key: &GroupKey) -> CryptoResult<Vec<u8>> {
    // Epoch aus AAD pruefen
    if payload.aad.len() >= 8 {
        let aad_epoch = u32::from_be_bytes(payload.aad[4..8].try_into().unwrap());
        if aad_epoch != key.epoch {
            return Err(CryptoError::EpochMismatch {
                erwartet: key.epoch,
                erhalten: aad_epoch,
            });
        }
    }

    match key.algorithm {
        GroupKeyAlgorithm::Aes256Gcm => decrypt_aes256gcm(
            &payload.ciphertext,
            key.key_bytes.as_bytes(),
            payload.nonce.as_bytes(),
            &payload.aad,
        ),
        GroupKeyAlgorithm::ChaCha20Poly1305 => decrypt_chacha20(
            &payload.ciphertext,
            key.key_bytes.as_bytes(),
            payload.nonce.as_bytes(),
            &payload.aad,
        ),
    }
}

/// Entschluesselt rohe Bytes (ohne EncryptedPayload-Wrapper)
///
/// Nuetzlich wenn Bytes direkt aus dem Netzwerk kommen.
pub fn decrypt_audio_bytes(data: &[u8], key: &GroupKey) -> CryptoResult<Vec<u8>> {
    let payload = EncryptedPayload::from_bytes(data)
        .ok_or_else(|| CryptoError::UngueltigeDaten("Ungueltige Payload-Struktur".to_string()))?;
    decrypt_audio(&payload, key)
}

fn decrypt_aes256gcm(
    ciphertext: &[u8],
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
        .decrypt(
            nonce,
            Payload {
                msg: ciphertext,
                aad,
            },
        )
        .map_err(|e| CryptoError::Entschluesselung(e.to_string()))
}

fn decrypt_chacha20(
    ciphertext: &[u8],
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
        .decrypt(
            nonce,
            chacha20poly1305::aead::Payload {
                msg: ciphertext,
                aad,
            },
        )
        .map_err(|e| CryptoError::Entschluesselung(e.to_string()))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::e2e::encrypt::encrypt_audio;
    use crate::e2e::group_key::create_group_key;

    #[test]
    fn roundtrip_aes256gcm() {
        let key = create_group_key("ch", 1, 0, GroupKeyAlgorithm::Aes256Gcm).unwrap();
        let plaintext = b"Hallo Opus-Audio 1234567890abcdef";

        let payload = encrypt_audio(plaintext, &key, 42, 1).unwrap();
        let decrypted = decrypt_audio(&payload, &key).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn roundtrip_chacha20() {
        let key = create_group_key("ch", 1, 0, GroupKeyAlgorithm::ChaCha20Poly1305).unwrap();
        let plaintext = b"ChaCha20-Test-Audio-Daten";

        let payload = encrypt_audio(plaintext, &key, 7, 3).unwrap();
        let decrypted = decrypt_audio(&payload, &key).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn falscher_schluessel_schlaegt_fehl() {
        let key1 = create_group_key("ch", 1, 0, GroupKeyAlgorithm::Aes256Gcm).unwrap();
        let key2 = create_group_key("ch", 2, 0, GroupKeyAlgorithm::Aes256Gcm).unwrap();
        let plaintext = b"Geheime Audio-Daten";

        let payload = encrypt_audio(plaintext, &key1, 1, 1).unwrap();
        let result = decrypt_audio(&payload, &key2);

        assert!(result.is_err());
    }

    #[test]
    fn manipulierter_ciphertext_schlaegt_fehl() {
        let key = create_group_key("ch", 1, 0, GroupKeyAlgorithm::Aes256Gcm).unwrap();
        let plaintext = b"Original-Audio";

        let mut payload = encrypt_audio(plaintext, &key, 1, 1).unwrap();
        // Ciphertext manipulieren
        if let Some(byte) = payload.ciphertext.first_mut() {
            *byte ^= 0xFF;
        }

        let result = decrypt_audio(&payload, &key);
        assert!(result.is_err());
    }

    #[test]
    fn epoch_mismatch_schlaegt_fehl() {
        let key_epoch0 = create_group_key("ch", 1, 0, GroupKeyAlgorithm::Aes256Gcm).unwrap();
        let key_epoch1 = create_group_key("ch", 2, 1, GroupKeyAlgorithm::Aes256Gcm).unwrap();

        let payload = encrypt_audio(b"audio", &key_epoch0, 1, 1).unwrap();
        // Mit Schluessel von Epoch 1 entschluesseln (AAD-Epoch ist 0)
        let result = decrypt_audio(&payload, &key_epoch1);

        assert!(matches!(result, Err(CryptoError::EpochMismatch { .. })));
    }

    #[test]
    fn decrypt_audio_bytes_roundtrip() {
        let key = create_group_key("ch", 1, 0, GroupKeyAlgorithm::Aes256Gcm).unwrap();
        let plaintext = b"Bytes-Roundtrip-Test";

        let payload = encrypt_audio(plaintext, &key, 5, 10).unwrap();
        let bytes = payload.to_bytes();

        let decrypted = decrypt_audio_bytes(&bytes, &key).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn leere_daten_entschluesseln() {
        let key = create_group_key("ch", 1, 0, GroupKeyAlgorithm::Aes256Gcm).unwrap();
        let plaintext = b"";

        let payload = encrypt_audio(plaintext, &key, 0, 0).unwrap();
        let decrypted = decrypt_audio(&payload, &key).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn ungueltige_bytes_schlagen_fehl() {
        let key = create_group_key("ch", 1, 0, GroupKeyAlgorithm::Aes256Gcm).unwrap();
        let result = decrypt_audio_bytes(&[0u8; 5], &key);
        assert!(result.is_err());
    }
}
