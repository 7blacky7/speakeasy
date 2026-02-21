//! Handler fuer alle Control-Nachrichten
//!
//! Jeder Handler ist fuer einen bestimmten Nachrichtentyp zustaendig
//! und hat Zugriff auf den gemeinsamen SignalingState.

pub mod auth_handler;
pub mod channel_handler;
pub mod chat_handler;
pub mod client_handler;
pub mod permission_handler;
pub mod server_handler;
pub mod voice_handler;
