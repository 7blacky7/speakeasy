//! PluginManager – Laden, Entladen und Lifecycle von Plugins
//!
//! Zentrale Komponente die alle anderen Teile des Plugin-Systems zusammenfuehrt.

use std::path::Path;
use std::sync::Arc;

use chrono::Utc;
use dashmap::DashMap;
use tracing::{info, warn};

use crate::error::{PluginError, Result};
use crate::events::{hook_ergebnisse_kombinieren, HookResult, PluginEvent};
use crate::host::capabilities::hat_faehigkeit;
use crate::host::sandbox::SandboxKonfiguration;
use crate::manifest::PluginManifest;
use crate::registry::PluginRegistry;
use crate::trust::trust_level_bestimmen;
use crate::types::{Plugin, PluginId, PluginInfo, PluginState, TrustLevel};

/// Konfiguration fuer den PluginManager
#[derive(Debug, Clone, Default)]
pub struct ManagerKonfiguration {
    /// Signierte Plugins sind Pflicht (unsignierte werden abgelehnt)
    pub signierung_erforderlich: bool,
    /// Verzeichnis in dem Plugins gesucht werden
    pub plugin_verzeichnis: Option<std::path::PathBuf>,
}

/// Interner Zustand eines geladenen Plugins
struct GeladenPlugin {
    plugin: Plugin,
    manifest: PluginManifest,
    #[allow(dead_code)]
    sandbox: SandboxKonfiguration,
}

/// PluginManager – verwaltet den gesamten Plugin-Lifecycle
pub struct PluginManager {
    registry: Arc<PluginRegistry>,
    plugins: DashMap<PluginId, GeladenPlugin>,
    konfiguration: ManagerKonfiguration,
}

impl PluginManager {
    /// Erstellt einen neuen PluginManager
    pub fn neu(konfiguration: ManagerKonfiguration) -> Self {
        Self {
            registry: Arc::new(PluginRegistry::neu()),
            plugins: DashMap::new(),
            konfiguration,
        }
    }

    /// Laedt ein Plugin aus einem Verzeichnis
    ///
    /// Erwartet ein Verzeichnis mit `manifest.toml` und der WASM-Datei.
    pub fn plugin_laden(&self, pfad: &Path) -> Result<PluginId> {
        // Manifest lesen
        let manifest_pfad = pfad.join("manifest.toml");
        let manifest = PluginManifest::from_file(&manifest_pfad)?;
        manifest.validieren()?;

        // WASM-Datei lesen
        let wasm_pfad = pfad.join(&manifest.plugin.wasm_file);
        let wasm_bytes = std::fs::read(&wasm_pfad)
            .map_err(|e| PluginError::WasmLaden(format!("{}: {}", wasm_pfad.display(), e)))?;

        // Optionale Signatur pruefen
        let signatur_pfad = pfad.join("plugin.sig");
        let signatur = std::fs::read(&signatur_pfad).ok();

        // Trust-Level bestimmen
        let trust_level = trust_level_bestimmen(&wasm_bytes, signatur.as_deref(), &[]);

        // Signierungspflicht pruefen
        if self.konfiguration.signierung_erforderlich && trust_level == TrustLevel::NichtSigniert {
            return Err(PluginError::NichtSigniert);
        }

        if trust_level == TrustLevel::NichtSigniert {
            warn!(
                "Plugin '{}' ist nicht signiert – Vorsicht!",
                manifest.plugin.name
            );
        }

        let id = PluginId::new();
        let sandbox = SandboxKonfiguration::aus_capabilities(&manifest.capabilities);

        let plugin_info = crate::types::PluginInfo {
            id,
            name: manifest.plugin.name.clone(),
            version: manifest.plugin.version.clone(),
            author: manifest.plugin.author.clone(),
            description: manifest.plugin.description.clone(),
            state: PluginState::Geladen,
            trust_level: trust_level.clone(),
            geladen_am: Utc::now(),
        };

        let plugin = Plugin::new(plugin_info, wasm_pfad, wasm_bytes);

        // In Registry eintragen
        self.registry.registrieren(
            id,
            manifest.plugin.name.clone(),
            manifest.plugin.version.clone(),
            pfad.to_path_buf(),
            trust_level,
        )?;

        self.plugins.insert(
            id,
            GeladenPlugin {
                plugin,
                manifest,
                sandbox,
            },
        );

        info!(
            "Plugin '{}' geladen (ID: {})",
            self.plugins.get(&id).unwrap().plugin.info.name,
            id
        );
        Ok(id)
    }

    /// Entlaedt ein Plugin vollstaendig
    pub fn plugin_entladen(&self, id: PluginId) -> Result<()> {
        let (_, geladen) = self
            .plugins
            .remove(&id)
            .ok_or_else(|| PluginError::NichtGefunden(id.to_string()))?;
        self.registry.entfernen(id)?;
        info!("Plugin '{}' entladen", geladen.plugin.info.name);
        Ok(())
    }

    /// Aktiviert ein geladenes Plugin
    pub fn plugin_aktivieren(&self, id: PluginId) -> Result<()> {
        self.registry.zustand_setzen(id, PluginState::Aktiv)?;
        let mut geladen = self
            .plugins
            .get_mut(&id)
            .ok_or_else(|| PluginError::NichtGefunden(id.to_string()))?;
        geladen.plugin.info.state = PluginState::Aktiv;
        info!("Plugin '{}' aktiviert", geladen.plugin.info.name);
        Ok(())
    }

    /// Deaktiviert ein aktives Plugin
    pub fn plugin_deaktivieren(&self, id: PluginId) -> Result<()> {
        self.registry.zustand_setzen(id, PluginState::Deaktiviert)?;
        let mut geladen = self
            .plugins
            .get_mut(&id)
            .ok_or_else(|| PluginError::NichtGefunden(id.to_string()))?;
        geladen.plugin.info.state = PluginState::Deaktiviert;
        info!("Plugin '{}' deaktiviert", geladen.plugin.info.name);
        Ok(())
    }

    /// Sendet ein Event an alle aktiven Plugins die es abonniert haben
    pub fn event_senden(&self, event: &PluginEvent) -> Result<()> {
        let event_name = event.name();
        let mut empfaenger = 0usize;

        for eintrag in self.plugins.iter() {
            let geladen = eintrag.value();
            if geladen.plugin.info.state != PluginState::Aktiv {
                continue;
            }
            // Pruefen ob Plugin dieses Event abonniert hat
            if !geladen
                .manifest
                .events
                .subscribe
                .iter()
                .any(|e| e == event_name)
            {
                continue;
            }
            // In echter Implementierung: WASM-Funktion aufrufen
            empfaenger += 1;
        }

        if empfaenger > 0 {
            tracing::debug!(
                "Event '{}' an {} Plugin(s) gesendet",
                event_name,
                empfaenger
            );
        }
        Ok(())
    }

    /// Fuehrt einen Hook aus und kombiniert die Ergebnisse
    ///
    /// Erstes `Deny` gewinnt. Alle aktiven Plugins mit dem Hook werden aufgerufen.
    pub fn hook_ausfuehren(&self, hook_name: &str, _daten: &[u8]) -> Result<HookResult> {
        let mut ergebnisse = Vec::new();

        for eintrag in self.plugins.iter() {
            let geladen = eintrag.value();
            if geladen.plugin.info.state != PluginState::Aktiv {
                continue;
            }
            // Pruefen ob Plugin diesen Hook hat
            let hat_hook = match hook_name {
                "before_chat_send" => geladen.manifest.hooks.before_chat_send,
                "after_user_join" => geladen.manifest.hooks.after_user_join,
                "before_user_kick" => geladen.manifest.hooks.before_user_kick,
                "before_channel_join" => geladen.manifest.hooks.before_channel_join,
                _ => false,
            };
            if !hat_hook {
                continue;
            }
            // In echter Implementierung: WASM-Funktion aufrufen
            // Hier simulieren wir Allow als Ergebnis
            ergebnisse.push(HookResult::Allow);
        }

        Ok(hook_ergebnisse_kombinieren(ergebnisse))
    }

    /// Gibt alle geladenen Plugins als PluginInfo-Liste zurueck
    pub fn plugins_auflisten(&self) -> Vec<PluginInfo> {
        self.plugins
            .iter()
            .map(|e| e.value().plugin.info.clone())
            .collect()
    }

    /// Gibt Informationen ueber ein spezifisches Plugin zurueck
    pub fn plugin_info(&self, id: PluginId) -> Option<PluginInfo> {
        self.plugins.get(&id).map(|e| e.plugin.info.clone())
    }

    /// Anzahl geladener Plugins
    pub fn anzahl_plugins(&self) -> usize {
        self.plugins.len()
    }

    /// Prueft ob ein Plugin eine bestimmte Capability hat
    pub fn hat_capability(&self, id: PluginId, capability: &str) -> bool {
        self.plugins
            .get(&id)
            .map(|e| hat_faehigkeit(&e.manifest.capabilities, capability))
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Hilfsfunktion: Erstellt ein temporaeres Plugin-Verzeichnis mit Manifest und Dummy-WASM
    fn erstelle_test_plugin(dir: &TempDir, name: &str, caps_toml: &str) -> std::path::PathBuf {
        let plugin_dir = dir.path().join(name);
        fs::create_dir_all(&plugin_dir).unwrap();

        let manifest = format!(
            r#"
[plugin]
name = "{name}"
version = "1.0.0"
author = "Test"
description = "Testplugin"
min_server_version = "0.1.0"
wasm_file = "plugin.wasm"

{caps_toml}

[events]
subscribe = ["user_join", "chat_message"]

[hooks]
before_chat_send = true
after_user_join = true
"#
        );
        fs::write(plugin_dir.join("manifest.toml"), manifest).unwrap();
        // Minimales gueltiges WAT-Modul als WASM-Bytes
        // \0asm = WASM Magic + Version 1
        let wasm_magic = b"\0asm\x01\x00\x00\x00";
        fs::write(plugin_dir.join("plugin.wasm"), wasm_magic).unwrap();

        plugin_dir
    }

    #[test]
    fn plugin_laden_und_auflisten() {
        let dir = TempDir::new().unwrap();
        let pfad = erstelle_test_plugin(&dir, "list-test", "[capabilities]\nchat_read = true");

        let manager = PluginManager::neu(ManagerKonfiguration::default());
        let id = manager.plugin_laden(&pfad).unwrap();

        let liste = manager.plugins_auflisten();
        assert_eq!(liste.len(), 1);
        assert_eq!(liste[0].id, id);
    }

    #[test]
    fn plugin_aktivieren_und_deaktivieren() {
        let dir = TempDir::new().unwrap();
        let pfad = erstelle_test_plugin(&dir, "lifecycle-test", "[capabilities]");

        let manager = PluginManager::neu(ManagerKonfiguration::default());
        let id = manager.plugin_laden(&pfad).unwrap();

        manager.plugin_aktivieren(id).unwrap();
        assert_eq!(manager.plugin_info(id).unwrap().state, PluginState::Aktiv);

        manager.plugin_deaktivieren(id).unwrap();
        assert_eq!(
            manager.plugin_info(id).unwrap().state,
            PluginState::Deaktiviert
        );
    }

    #[test]
    fn plugin_entladen() {
        let dir = TempDir::new().unwrap();
        let pfad = erstelle_test_plugin(&dir, "unload-test", "[capabilities]");

        let manager = PluginManager::neu(ManagerKonfiguration::default());
        let id = manager.plugin_laden(&pfad).unwrap();
        assert_eq!(manager.anzahl_plugins(), 1);

        manager.plugin_entladen(id).unwrap();
        assert_eq!(manager.anzahl_plugins(), 0);
    }

    #[test]
    fn entladen_nicht_gefunden() {
        let manager = PluginManager::neu(ManagerKonfiguration::default());
        let err = manager.plugin_entladen(PluginId::new()).unwrap_err();
        assert!(matches!(err, PluginError::NichtGefunden(_)));
    }

    #[test]
    fn event_senden_an_aktive_plugins() {
        let dir = TempDir::new().unwrap();
        let pfad = erstelle_test_plugin(&dir, "event-test", "[capabilities]\nchat_read = true");

        let manager = PluginManager::neu(ManagerKonfiguration::default());
        let id = manager.plugin_laden(&pfad).unwrap();
        manager.plugin_aktivieren(id).unwrap();

        let event = PluginEvent::ChatMessage {
            channel_id: "c1".into(),
            sender_id: "u1".into(),
            content: "Hallo".into(),
        };
        assert!(manager.event_senden(&event).is_ok());
    }

    #[test]
    fn hook_ausfuehren_allow() {
        let dir = TempDir::new().unwrap();
        let pfad = erstelle_test_plugin(&dir, "hook-test", "[capabilities]\nchat_write = true");

        let manager = PluginManager::neu(ManagerKonfiguration::default());
        let id = manager.plugin_laden(&pfad).unwrap();
        manager.plugin_aktivieren(id).unwrap();

        let result = manager
            .hook_ausfuehren("before_chat_send", b"Hallo")
            .unwrap();
        assert!(result.ist_erlaubt());
    }

    #[test]
    fn signierung_erforderlich_abgelehnt() {
        let dir = TempDir::new().unwrap();
        let pfad = erstelle_test_plugin(&dir, "unsigned-test", "[capabilities]");

        let config = ManagerKonfiguration {
            signierung_erforderlich: true,
            ..Default::default()
        };
        let manager = PluginManager::neu(config);
        let err = manager.plugin_laden(&pfad).unwrap_err();
        assert!(matches!(err, PluginError::NichtSigniert));
    }

    #[test]
    fn capability_pruefen() {
        let dir = TempDir::new().unwrap();
        let pfad = erstelle_test_plugin(&dir, "cap-test", "[capabilities]\nchat_read = true");

        let manager = PluginManager::neu(ManagerKonfiguration::default());
        let id = manager.plugin_laden(&pfad).unwrap();

        assert!(manager.hat_capability(id, "chat_read"));
        assert!(!manager.hat_capability(id, "network"));
    }
}
