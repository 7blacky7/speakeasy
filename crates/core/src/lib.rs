//! speakeasy-core – Gemeinsame Typen, Traits und Fehlertypen
//!
//! Dieses Crate stellt die fundamentalen Bausteine bereit, die von allen
//! anderen Speakeasy-Crates gemeinsam genutzt werden.

pub mod error;
pub mod event;
pub mod types;

// Re-Exporte fuer bequemen Zugriff
pub use error::{Result, SpeakeasyError};
pub use types::{ChannelId, ServerId, UserId};
