use std::sync::Mutex;

use speakeasy_audio::engine::AudioEngineConfig;
use speakeasy_plugin::manager::{ManagerKonfiguration, PluginManager};

/// Verbindungszustand des Clients
#[derive(Debug, Default)]
pub struct ConnectionState {
    pub connected: bool,
    pub server_address: Option<String>,
    pub server_port: Option<u16>,
    pub username: Option<String>,
    pub current_channel: Option<String>,
}

/// Audio-Zustand
#[derive(Debug, Default)]
pub struct AudioState {
    pub muted: bool,
    pub deafened: bool,
    /// Aktuelle Audio-Engine-Konfiguration (gespeicherte Einstellungen)
    pub engine_config: Option<AudioEngineConfig>,
}

/// Globaler Anwendungszustand (Mutex-gesichert fuer Thread-Sicherheit)
#[derive(Default)]
pub struct AppState {
    pub connection: Mutex<ConnectionState>,
    pub audio: Mutex<AudioState>,
    pub plugin_manager: Mutex<Option<PluginManager>>,
}

impl AppState {
    /// Erstellt einen neuen AppState mit initialisiertem PluginManager
    pub fn mit_plugins() -> Self {
        let manager = PluginManager::neu(ManagerKonfiguration::default());
        Self {
            connection: Mutex::new(ConnectionState::default()),
            audio: Mutex::new(AudioState::default()),
            plugin_manager: Mutex::new(Some(manager)),
        }
    }
}
