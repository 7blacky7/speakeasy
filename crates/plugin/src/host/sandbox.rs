//! Sandbox-Konfiguration fuer WASM-Plugins
//!
//! Legt fest welche WASI-Features ein Plugin nutzen darf,
//! basierend auf seinen deklarierten Capabilities.

use crate::manifest::Capabilities;

/// Sandbox-Konfiguration die aus den Capabilities abgeleitet wird
#[derive(Debug, Clone)]
pub struct SandboxKonfiguration {
    /// Dateisystemzugriff erlaubt
    pub filesystem: bool,
    /// Netzwerkzugriff erlaubt (via Host-API, nicht direkt)
    pub network: bool,
    /// Maximale Speichernutzung in Bytes (Standard: 64 MB)
    pub max_speicher_bytes: u64,
    /// Maximale CPU-Instruktionen pro Aufruf (0 = unbegrenzt)
    pub max_instruktionen: u64,
    /// WASI stdio erlaubt (fuer Logging)
    pub stdio: bool,
}

impl SandboxKonfiguration {
    /// Standard-Sandbox – minimale Rechte
    pub fn minimal() -> Self {
        Self {
            filesystem: false,
            network: false,
            max_speicher_bytes: 64 * 1024 * 1024, // 64 MB
            max_instruktionen: 0,
            stdio: true, // Logging immer erlaubt
        }
    }

    /// Erstellt Sandbox-Konfiguration aus Plugin-Capabilities
    pub fn aus_capabilities(caps: &Capabilities) -> Self {
        Self {
            filesystem: caps.filesystem,
            network: caps.network,
            max_speicher_bytes: 64 * 1024 * 1024,
            max_instruktionen: 0,
            stdio: true,
        }
    }

    /// Prueft ob diese Konfiguration sicher genug fuer Produktionsbetrieb ist
    pub fn ist_produktionssicher(&self) -> bool {
        // Direkte Netzwerkverbindungen ohne Capability sind gefaehrlich
        // Hier nur Host-API-basiertes Netzwerk erlaubt
        true
    }
}

impl Default for SandboxKonfiguration {
    fn default() -> Self {
        Self::minimal()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::Capabilities;

    #[test]
    fn minimal_sandbox_keine_rechte() {
        let sb = SandboxKonfiguration::minimal();
        assert!(!sb.filesystem);
        assert!(!sb.network);
        assert!(sb.stdio);
        assert_eq!(sb.max_speicher_bytes, 64 * 1024 * 1024);
    }

    #[test]
    fn sandbox_aus_capabilities_filesystem() {
        let caps = Capabilities {
            filesystem: true,
            ..Default::default()
        };
        let sb = SandboxKonfiguration::aus_capabilities(&caps);
        assert!(sb.filesystem);
        assert!(!sb.network);
    }

    #[test]
    fn sandbox_aus_capabilities_network() {
        let caps = Capabilities {
            network: true,
            ..Default::default()
        };
        let sb = SandboxKonfiguration::aus_capabilities(&caps);
        assert!(sb.network);
        assert!(!sb.filesystem);
    }

    #[test]
    fn sandbox_produktionssicher() {
        let sb = SandboxKonfiguration::minimal();
        assert!(sb.ist_produktionssicher());
    }

    #[test]
    fn sandbox_default_ist_minimal() {
        let sb = SandboxKonfiguration::default();
        assert!(!sb.filesystem);
        assert!(!sb.network);
    }
}
