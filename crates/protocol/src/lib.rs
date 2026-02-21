//! speakeasy-protocol – Netzwerkprotokoll-Definitionen
//!
//! Dieses Crate definiert alle Nachrichtentypen, Enums und Strukturen
//! die zwischen Client und Server ausgetauscht werden.

pub mod control;
pub mod voice;

pub use control::{CommandType, ControlMessage, MessageType};
pub use voice::VoicePaket;
