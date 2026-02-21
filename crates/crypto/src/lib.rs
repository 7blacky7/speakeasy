//! # speakeasy-crypto
//!
//! E2E Verschluesselung und DTLS-Subsystem fuer Speakeasy.
//!
//! ## Module
//! - `dtls` - TLS/DTLS Transport-Verschluesselung (Client <-> Server)
//! - `e2e` - Ende-zu-Ende Verschluesselung (Client <-> Client)
//! - `identity` - Ed25519 Langzeit-Identitaetsschluessel
//! - `types` - Gemeinsame Typen (KeyPair, Nonce, GroupKey, etc.)
//! - `error` - Fehlertypen

pub mod dtls;
pub mod e2e;
pub mod error;
pub mod identity;
pub mod types;

// Bequeme Re-Exports
pub use error::{CryptoError, CryptoResult};
pub use identity::{Identity, PublicIdentity};
pub use types::{EncryptedPayload, GroupKey, GroupKeyAlgorithm, Nonce, PublicKey, SecretBytes};

pub use e2e::{
    decrypt_audio, decrypt_audio_bytes, encrypt_audio, hkdf_derive, wrap_key_for_recipient,
    unwrap_key_for_recipient, create_group_key, rotate_group_key,
    GroupKeyManager, KeyExchangeClient, KeyExchangeServer, SharedSecret,
};

pub use dtls::{
    compute_certificate_fingerprint, generate_self_signed_cert, DtlsClient, DtlsClientConfig,
    DtlsServer, DtlsServerConfig,
};
