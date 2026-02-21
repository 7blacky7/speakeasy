//! Grundlegende Typen fuer das Plugin-System

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Eindeutige Plugin-ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PluginId(pub Uuid);

impl PluginId {
    /// Erstellt eine neue zufaellige PluginId
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Gibt die innere UUID zurueck
    pub fn inner(&self) -> Uuid {
        self.0
    }
}

impl Default for PluginId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for PluginId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "plugin:{}", self.0)
    }
}

/// Zustand eines Plugins
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginState {
    /// Plugin ist geladen aber nicht aktiv
    Geladen,
    /// Plugin ist aktiv und laufbereit
    Aktiv,
    /// Plugin ist deaktiviert
    Deaktiviert,
    /// Plugin hat einen Fehler
    Fehler(String),
}

impl std::fmt::Display for PluginState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PluginState::Geladen => write!(f, "Geladen"),
            PluginState::Aktiv => write!(f, "Aktiv"),
            PluginState::Deaktiviert => write!(f, "Deaktiviert"),
            PluginState::Fehler(e) => write!(f, "Fehler: {}", e),
        }
    }
}

/// Trust-Level eines Plugins
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrustLevel {
    /// Nicht signiert – Warnung beim Laden
    NichtSigniert,
    /// Signiert mit bekanntem Schluessel
    Signiert,
    /// Vertrauenswuerdig – automatisches Laden erlaubt
    Vertrauenswuerdig,
}

impl std::fmt::Display for TrustLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrustLevel::NichtSigniert => write!(f, "Nicht signiert"),
            TrustLevel::Signiert => write!(f, "Signiert"),
            TrustLevel::Vertrauenswuerdig => write!(f, "Vertrauenswuerdig"),
        }
    }
}

/// Oeffentliche Informationen ueber ein Plugin (fuer UI/API)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    pub id: PluginId,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub state: PluginState,
    pub trust_level: TrustLevel,
    pub geladen_am: DateTime<Utc>,
}

/// Interner Plugin-Container mit WASM-Instanz
#[derive(Debug)]
pub struct Plugin {
    pub info: PluginInfo,
    pub wasm_pfad: std::path::PathBuf,
    pub wasm_bytes: Vec<u8>,
}

impl Plugin {
    pub fn new(info: PluginInfo, wasm_pfad: std::path::PathBuf, wasm_bytes: Vec<u8>) -> Self {
        Self {
            info,
            wasm_pfad,
            wasm_bytes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_id_eindeutig() {
        let a = PluginId::new();
        let b = PluginId::new();
        assert_ne!(a, b);
    }

    #[test]
    fn plugin_id_display() {
        let id = PluginId(Uuid::nil());
        assert!(id.to_string().starts_with("plugin:"));
    }

    #[test]
    fn plugin_state_anzeige() {
        assert_eq!(PluginState::Aktiv.to_string(), "Aktiv");
        assert_eq!(PluginState::Deaktiviert.to_string(), "Deaktiviert");
        let e = PluginState::Fehler("Absturz".into());
        assert!(e.to_string().contains("Fehler"));
    }

    #[test]
    fn trust_level_anzeige() {
        assert_eq!(TrustLevel::NichtSigniert.to_string(), "Nicht signiert");
        assert_eq!(
            TrustLevel::Vertrauenswuerdig.to_string(),
            "Vertrauenswuerdig"
        );
    }

    #[test]
    fn plugin_info_serde() {
        let info = PluginInfo {
            id: PluginId::new(),
            name: "test-plugin".into(),
            version: "1.0.0".into(),
            author: "Test".into(),
            description: "Testplugin".into(),
            state: PluginState::Geladen,
            trust_level: TrustLevel::NichtSigniert,
            geladen_am: Utc::now(),
        };
        let json = serde_json::to_string(&info).unwrap();
        let info2: PluginInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(info.id, info2.id);
        assert_eq!(info.name, info2.name);
    }
}
