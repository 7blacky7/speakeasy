//! speakeasy-plugin – WASM Plugin System
//!
//! Dieses Crate implementiert das Plugin-System fuer Speakeasy-Server und -Client.
//! Plugins werden als WASM-Module geladen und in einer Sandbox ausgefuehrt.
//!
//! # Architektur
//! - [`manager::PluginManager`] – Laden, Entladen, Lifecycle
//! - [`manifest::PluginManifest`] – Plugin-Metadaten und Konfiguration
//! - [`events::PluginEvent`] – Event-System fuer Plugin-Hooks
//! - [`trust`] – Ed25519 Signierung und Verifikation
//! - [`registry::PluginRegistry`] – Installierte Plugins verwalten
//! - [`host`] – WASM Runtime und Host-API

pub mod error;
pub mod events;
pub mod host;
pub mod manager;
pub mod manifest;
pub mod registry;
pub mod trust;
pub mod types;

// Bequeme Re-Exporte
pub use error::{PluginError, Result};
pub use events::{HookResult, PluginEvent};
pub use manager::{ManagerKonfiguration, PluginManager};
pub use manifest::PluginManifest;
pub use registry::PluginRegistry;
pub use types::{PluginId, PluginInfo, PluginState, TrustLevel};
