//! speakeasy-db – Datenbank-Abstraktion
//!
//! Dieses Crate stellt das Repository-Pattern bereit, das SQLite (Standard)
//! und PostgreSQL (Multi-Instance) hinter einer einheitlichen Schnittstelle
//! abstrahiert.
//!
//! # Verwendung
//!
//! ```no_run
//! use speakeasy_db::{SqliteDb, repository::{DatabaseConfig, UserRepository}};
//! use speakeasy_db::models::NeuerBenutzer;
//!
//! #[tokio::main]
//! async fn main() {
//!     let cfg = DatabaseConfig::default();
//!     let db = SqliteDb::oeffnen(&cfg).await.unwrap();
//!
//!     let user = db.create(NeuerBenutzer {
//!         username: "admin",
//!         password_hash: "hash",
//!     }).await.unwrap();
//!
//!     println!("Benutzer erstellt: {}", user.username);
//! }
//! ```

pub mod error;
pub mod models;
pub mod permissions;
pub mod repository;
pub mod sqlite;

// Bequeme Re-Exporte
pub use error::DbError;
pub use repository::{
    AuditLogRepository, BanRepository, ChannelGroupRepository, ChannelRepository,
    DatabaseBackend, DatabaseConfig, DbResult, InviteRepository, PermissionRepository,
    ServerGroupRepository, UserRepository,
};
pub use sqlite::SqliteDb;
