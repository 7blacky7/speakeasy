//! speakeasy-db – Datenbank-Abstraktion
//!
//! Dieses Crate stellt das Repository-Pattern bereit, das SQLite (Standard)
//! und PostgreSQL (Multi-Instance) hinter einer einheitlichen Schnittstelle
//! abstrahiert. Die konkreten Implementierungen werden in einem spaeteren
//! Task ergaenzt.

pub mod repository;

pub use repository::{
    BenutzerRepository, DatabaseBackend, DatabaseConfig, KanalRepository,
};
