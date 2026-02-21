//! speakeasy-voice – Voice-Forwarding-Engine
//!
//! Implementiert den Media Router (SFU-Stil) fuer Echtzeit-Audio-Uebertragung.
//!
//! ## Module
//! - [`udp`] – UDP-Listener und Send-Queue pro Client
//! - [`router`] – Channel-Router fuer Paket-Weiterleitung
//! - [`jitter_buffer`] – Adaptiver Jitter Buffer
//! - [`congestion`] – Congestion Controller mit Bitrate-Adaptation
//! - [`state`] – In-Memory Voice-State aller Sessions
//! - [`telemetry`] – Quality-Telemetrie und Metriken
//! - [`plc`] – Packet Loss Concealment

pub mod congestion;
pub mod jitter_buffer;
pub mod plc;
pub mod router;
pub mod state;
pub mod telemetry;
pub mod udp;

pub use router::ChannelRouter;
pub use state::VoiceState;
pub use udp::VoiceServer;
