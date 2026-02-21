//! Plugin-Manifest Parsing (manifest.toml)
//!
//! Jedes Plugin liefert eine manifest.toml die Metadaten,
//! erforderliche Capabilities und abonnierte Events beschreibt.

use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::error::{PluginError, Result};

/// Vollstaendiges Plugin-Manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub plugin: PluginMeta,
    pub capabilities: Capabilities,
    #[serde(default)]
    pub events: EventConfig,
    #[serde(default)]
    pub hooks: HookConfig,
}

/// Plugin-Metadaten
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMeta {
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub min_server_version: String,
    pub wasm_file: String,
}

/// Capability-Konfiguration – welche Zugriffe das Plugin benoetigt
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Capabilities {
    /// WASI Dateisystem-Zugriff
    #[serde(default)]
    pub filesystem: bool,
    /// Netzwerkzugriff via Host-API
    #[serde(default)]
    pub network: bool,
    /// Audio-Stream lesen
    #[serde(default)]
    pub audio_read: bool,
    /// Audio-Stream modifizieren
    #[serde(default)]
    pub audio_write: bool,
    /// Chat-Nachrichten lesen
    #[serde(default)]
    pub chat_read: bool,
    /// Chat-Nachrichten senden
    #[serde(default)]
    pub chat_write: bool,
    /// User kick/ban/move
    #[serde(default)]
    pub user_management: bool,
    /// Server-Konfiguration aendern
    #[serde(default)]
    pub server_config: bool,
}

/// Events die das Plugin abonniert
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EventConfig {
    #[serde(default)]
    pub subscribe: Vec<String>,
}

/// Hook-Konfiguration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HookConfig {
    #[serde(default)]
    pub before_chat_send: bool,
    #[serde(default)]
    pub after_user_join: bool,
    #[serde(default)]
    pub before_user_kick: bool,
    #[serde(default)]
    pub before_channel_join: bool,
}

impl PluginManifest {
    /// Laedt ein Manifest aus einer TOML-Datei
    pub fn from_file(pfad: &Path) -> Result<Self> {
        let inhalt = std::fs::read_to_string(pfad).map_err(|e| {
            PluginError::ManifestNichtGefunden(format!("{}: {}", pfad.display(), e))
        })?;
        Self::parse(&inhalt)
    }

    /// Parst ein Manifest aus einem TOML-String
    pub fn parse(inhalt: &str) -> Result<Self> {
        toml::from_str(inhalt).map_err(|e| PluginError::Manifest(e.to_string()))
    }

    /// Validiert das Manifest auf Pflichtfelder und Konsistenz
    pub fn validieren(&self) -> Result<()> {
        if self.plugin.name.is_empty() {
            return Err(PluginError::Manifest("Pflichtfeld fehlt: plugin.name".into()));
        }
        if self.plugin.version.is_empty() {
            return Err(PluginError::Manifest("Pflichtfeld fehlt: plugin.version".into()));
        }
        if self.plugin.wasm_file.is_empty() {
            return Err(PluginError::Manifest("Pflichtfeld fehlt: plugin.wasm_file".into()));
        }
        // Version muss semver-kompatibel sein (x.y.z)
        if !ist_semver(&self.plugin.version) {
            return Err(PluginError::UngueltigeVersion(self.plugin.version.clone()));
        }
        Ok(())
    }
}

/// Einfache Pruefung ob ein String semver-Format hat (x.y.z)
fn ist_semver(v: &str) -> bool {
    let teile: Vec<&str> = v.split('.').collect();
    if teile.len() != 3 {
        return false;
    }
    teile.iter().all(|t| t.parse::<u32>().is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    const GUELTIG_TOML: &str = r#"
[plugin]
name = "test-plugin"
version = "1.0.0"
author = "Test Autor"
description = "Ein Testplugin"
min_server_version = "0.1.0"
wasm_file = "plugin.wasm"

[capabilities]
chat_read = true
chat_write = true

[events]
subscribe = ["user_join", "chat_message"]

[hooks]
before_chat_send = true
"#;

    #[test]
    fn manifest_parsing_gueltig() {
        let m = PluginManifest::parse(GUELTIG_TOML).unwrap();
        assert_eq!(m.plugin.name, "test-plugin");
        assert_eq!(m.plugin.version, "1.0.0");
        assert!(m.capabilities.chat_read);
        assert!(m.capabilities.chat_write);
        assert!(!m.capabilities.network);
        assert_eq!(m.events.subscribe.len(), 2);
        assert!(m.hooks.before_chat_send);
    }

    #[test]
    fn manifest_validierung_ok() {
        let m = PluginManifest::parse(GUELTIG_TOML).unwrap();
        assert!(m.validieren().is_ok());
    }

    #[test]
    fn manifest_validierung_fehlender_name() {
        let toml = r#"
[plugin]
name = ""
version = "1.0.0"
author = "Test"
description = "Test"
min_server_version = "0.1.0"
wasm_file = "plugin.wasm"

[capabilities]
"#;
        let m = PluginManifest::parse(toml).unwrap();
        let err = m.validieren().unwrap_err();
        assert!(err.to_string().contains("plugin.name"));
    }

    #[test]
    fn manifest_validierung_ungueltige_version() {
        let toml = r#"
[plugin]
name = "test"
version = "nicht-semver"
author = "Test"
description = "Test"
min_server_version = "0.1.0"
wasm_file = "plugin.wasm"

[capabilities]
"#;
        let m = PluginManifest::parse(toml).unwrap();
        let err = m.validieren().unwrap_err();
        assert!(matches!(err, PluginError::UngueltigeVersion(_)));
    }

    #[test]
    fn manifest_parsing_ungueltig() {
        let err = PluginManifest::parse("das ist kein toml :::").unwrap_err();
        assert!(matches!(err, PluginError::Manifest(_)));
    }

    #[test]
    fn manifest_defaults() {
        let m = PluginManifest::parse(GUELTIG_TOML).unwrap();
        // Nicht gesetzte Capabilities sind false
        assert!(!m.capabilities.filesystem);
        assert!(!m.capabilities.audio_read);
        assert!(!m.capabilities.user_management);
        // Nicht gesetzte Hooks sind false
        assert!(!m.hooks.after_user_join);
    }

    #[test]
    fn semver_pruefung() {
        assert!(ist_semver("1.0.0"));
        assert!(ist_semver("0.1.23"));
        assert!(!ist_semver("1.0"));
        assert!(!ist_semver("abc"));
        assert!(!ist_semver("1.0.0.0"));
    }

    #[test]
    fn manifest_datei_nicht_gefunden() {
        let err = PluginManifest::from_file(Path::new("/existiert/nicht/manifest.toml"))
            .unwrap_err();
        assert!(matches!(err, PluginError::ManifestNichtGefunden(_)));
    }
}
