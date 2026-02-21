//! speakeasy-chat – Text-Chat und Dateiversand
//!
//! Dieses Crate implementiert:
//! - ChatService: Nachrichten senden, editieren, loeschen, History, Suche
//! - FileService: Datei-Upload/Download mit Quota-Pruefung und SHA-256
//! - StorageBackend-Trait + DiskStorage-Implementierung
//!
//! # Beispiel
//!
//! ```no_run
//! use std::sync::Arc;
//! use speakeasy_chat::{ChatService, FileService, DiskStorage};
//! use speakeasy_db::SqliteDb;
//!
//! #[tokio::main]
//! async fn main() {
//!     // DB-Verbindung
//!     let db = Arc::new(SqliteDb::in_memory().await.unwrap());
//!
//!     // ChatService
//!     let chat = ChatService::neu(db.clone());
//!
//!     // FileService mit DiskStorage
//!     let storage = Arc::new(DiskStorage::new("data/files"));
//!     let files = FileService::neu(db.clone(), db.clone(), storage);
//! }
//! ```

pub mod error;
pub mod file_service;
pub mod service;
pub mod storage;
pub mod types;

#[cfg(test)]
mod tests;

// Bequeme Re-Exporte
pub use error::{ChatError, ChatResult};
pub use file_service::FileService;
pub use service::ChatService;
pub use storage::{DiskStorage, StorageBackend};
pub use types::{ChatNachricht, DateeiInfo, DateiUpload, HistoryAnfrage, NachrichtenTyp};
