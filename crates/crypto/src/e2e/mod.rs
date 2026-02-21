//! E2E Verschluesselung (End-to-End)
//!
//! Client <-> Client Verschluesselung. Der Server forwardet Pakete blind
//! und kann den Audio-Inhalt nicht entschluesseln.
//!
//! ## Ablauf
//! 1. Jeder Client hat eine `Identity` (Ed25519 Langzeit-Key)
//! 2. Bei Channel-Beitritt: X25519 Key Exchange mit dem Key-Verteiler
//! 3. Gruppen-Schluessel wird sicher an alle Mitglieder verteilt
//! 4. Audio wird mit dem Gruppen-Schluessel (AES-256-GCM) verschluesselt
//! 5. Bei Join/Leave: Key Rotation (neue Epoch)

pub mod decrypt;
pub mod encrypt;
pub mod group_key;
pub mod key_exchange;
pub mod key_manager;

pub use decrypt::{decrypt_audio, decrypt_audio_bytes};
pub use encrypt::encrypt_audio;
pub use group_key::{
    create_group_key, rotate_group_key, unwrap_key_for_recipient, wrap_key_for_recipient,
};
pub use key_exchange::{hkdf_derive, KeyExchangeClient, KeyExchangeServer, SharedSecret};
pub use key_manager::GroupKeyManager;
