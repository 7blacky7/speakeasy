//! Gemeinsame Typen fuer das Kryptografie-Subsystem

use serde::{Deserialize, Serialize};

/// Ein kryptografisches Schluessel-Paar (oeffentlich + privat)
#[derive(Debug, Clone)]
pub struct KeyPair {
    /// Privater Schluessel (32 Bytes fuer X25519)
    pub private_key: Vec<u8>,
    /// Oeffentlicher Schluessel (32 Bytes fuer X25519)
    pub public_key: PublicKey,
}

/// Oeffentlicher Schluessel
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicKey {
    pub bytes: Vec<u8>,
}

impl PublicKey {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Eine kryptografische Nonce (Number used once)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Nonce {
    pub bytes: [u8; 12],
}

impl Nonce {
    /// Erstellt eine Nonce aus Epoch + Sequenz-Nummer + Zufalls-Bytes
    pub fn aus_epoch_und_seq(epoch: u32, seq: u32, random: [u8; 4]) -> Self {
        let mut bytes = [0u8; 12];
        bytes[0..4].copy_from_slice(&epoch.to_be_bytes());
        bytes[4..8].copy_from_slice(&seq.to_be_bytes());
        bytes[8..12].copy_from_slice(&random);
        Self { bytes }
    }

    pub fn as_bytes(&self) -> &[u8; 12] {
        &self.bytes
    }

    /// Liest die Epoch aus der Nonce
    pub fn epoch(&self) -> u32 {
        u32::from_be_bytes(self.bytes[0..4].try_into().unwrap())
    }

    /// Liest die Sequenz-Nummer aus der Nonce
    pub fn seq(&self) -> u32 {
        u32::from_be_bytes(self.bytes[4..8].try_into().unwrap())
    }
}

/// Symmetrischer Gruppen-Schluessel fuer einen Channel
#[derive(Debug, Clone)]
pub struct GroupKey {
    /// Eindeutige Schluessel-ID (monoton steigend)
    pub key_id: u64,
    /// Epoch-Nummer (erhoehbar bei Rotation)
    pub epoch: u32,
    /// Der eigentliche Schluessel (32 Bytes fuer AES-256-GCM / ChaCha20-Poly1305)
    pub key_bytes: SecretBytes,
    /// Algorithmus
    pub algorithm: GroupKeyAlgorithm,
    /// Channel-ID
    pub channel_id: String,
}

/// Sicherer Schluessel-Container (wird beim Drop genullt)
#[derive(Clone)]
pub struct SecretBytes(pub Vec<u8>);

impl Drop for SecretBytes {
    fn drop(&mut self) {
        self.0.iter_mut().for_each(|b| *b = 0);
    }
}

impl std::fmt::Debug for SecretBytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SecretBytes([REDACTED] {} bytes)", self.0.len())
    }
}

impl SecretBytes {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Algorithmus fuer Gruppen-Schluessel
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[derive(Default)]
pub enum GroupKeyAlgorithm {
    #[default]
    Aes256Gcm,
    ChaCha20Poly1305,
}


/// Verschluesselter Payload (Nonce + Ciphertext + Auth-Tag)
#[derive(Debug, Clone)]
pub struct EncryptedPayload {
    /// 12 Bytes Nonce
    pub nonce: Nonce,
    /// Verschluesselter Inhalt inkl. 16 Bytes Auth-Tag (angehaengt)
    pub ciphertext: Vec<u8>,
    /// Authenticated Additional Data (SSRC + Epoch)
    pub aad: Vec<u8>,
}

impl EncryptedPayload {
    /// Serialisiert zu Bytes: [nonce(12)] + [aad_len(2)] + [aad] + [ciphertext]
    pub fn to_bytes(&self) -> Vec<u8> {
        let aad_len = self.aad.len() as u16;
        let mut out = Vec::with_capacity(12 + 2 + self.aad.len() + self.ciphertext.len());
        out.extend_from_slice(&self.nonce.bytes);
        out.extend_from_slice(&aad_len.to_be_bytes());
        out.extend_from_slice(&self.aad);
        out.extend_from_slice(&self.ciphertext);
        out
    }

    /// Deserialisiert aus Bytes
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 14 {
            return None;
        }
        let mut nonce_bytes = [0u8; 12];
        nonce_bytes.copy_from_slice(&bytes[0..12]);
        let nonce = Nonce { bytes: nonce_bytes };

        let aad_len = u16::from_be_bytes([bytes[12], bytes[13]]) as usize;
        if bytes.len() < 14 + aad_len {
            return None;
        }
        let aad = bytes[14..14 + aad_len].to_vec();
        let ciphertext = bytes[14 + aad_len..].to_vec();

        Some(Self { nonce, ciphertext, aad })
    }
}
