//! speakeasy-protocol – Netzwerkprotokoll-Definitionen
//!
//! Dieses Crate definiert alle Nachrichtentypen, Enums und Strukturen
//! die zwischen Client und Server ausgetauscht werden.
//!
//! ## Module
//! - `voice`  – UDP Voice-Pakete (binaer, kein serde, performance-kritisch)
//! - `control` – TCP Control-Nachrichten (JSON via serde)
//! - `crypto`  – DTLS/E2E Krypto-Typen (Implementierung in Phase 5)
//! - `codec`   – Opus-Konfiguration und Audio-Presets
//! - `wire`    – TCP Frame-Codec (tokio-util Encoder/Decoder)

pub mod codec;
pub mod control;
pub mod crypto;
pub mod voice;
pub mod wire;

// Re-Exporte fuer bequemen Zugriff
pub use codec::{AudioPreset, CodecNegotiationRequest, CodecNegotiationResponse, OpusConfig};
pub use control::{ControlMessage, ControlPayload, ErrorCode, ErrorResponse};
pub use crypto::{CryptoMode, E2EKeyMessage, KeyExchangeMessage};
pub use voice::{PacketType, VoiceFlags, VoicePacket, VoicePacketHeader, VoicePaket};
pub use wire::{FrameCodec, DEFAULT_MAX_FRAME_SIZE};
