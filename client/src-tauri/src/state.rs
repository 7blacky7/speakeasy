use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use speakeasy_audio::engine::AudioEngineConfig;
use speakeasy_plugin::manager::{ManagerKonfiguration, PluginManager};
use tokio::sync::Mutex as AsyncMutex;

use crate::connection::ServerConnection;
use crate::voice::VoiceClient;

/// Verbindungszustand des Clients (leichtgewichtige Metadaten)
#[derive(Debug, Default)]
pub struct ConnectionState {
    pub connected: bool,
    pub server_address: Option<String>,
    pub server_port: Option<u16>,
    pub username: Option<String>,
    pub current_channel: Option<String>,
    /// Ob der Benutzer sein Passwort zwingend aendern muss
    pub force_password_change: bool,
}

/// Echtzeit-Pegel vom Audio-Monitor (lock-free lesbar)
#[derive(Debug, Default)]
pub struct MonitorLevels {
    /// RMS-Eingangspegel (0.0 - 1.0)
    pub input_level: f32,
    /// RMS-Pegel nach DSP (0.0 - 1.0) - aktuell gleich input_level
    pub processed_level: f32,
    /// Noise Floor in dBFS
    pub noise_floor: f32,
    /// Clipping erkannt
    pub is_clipping: bool,
}

/// Audio-Monitor Handle (der cpal-Stream lebt in einem dedizierten Thread)
#[derive(Debug)]
pub struct AudioMonitor {
    pub levels: Arc<Mutex<MonitorLevels>>,
    pub running: Arc<AtomicBool>,
}

impl AudioMonitor {
    pub fn new(levels: Arc<Mutex<MonitorLevels>>, running: Arc<AtomicBool>) -> Self {
        Self { levels, running }
    }
}

impl Drop for AudioMonitor {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
    }
}

/// Audio-Zustand
#[derive(Debug, Default)]
pub struct AudioState {
    pub muted: bool,
    pub deafened: bool,
    /// Aktuelle Audio-Engine-Konfiguration (gespeicherte Einstellungen)
    pub engine_config: Option<AudioEngineConfig>,
    /// Vollstaendige Audio-Einstellungen vom Frontend (inkl. DSP, Codec, Jitter)
    pub full_settings: Option<crate::commands::AudioSettingsConfig>,
    /// Aktiver Audio-Monitor fuer Echtzeit-Pegel
    pub monitor: Option<AudioMonitor>,
}

/// Globaler Anwendungszustand (Mutex-gesichert fuer Thread-Sicherheit)
pub struct AppState {
    /// Leichtgewichtige Verbindungs-Metadaten (sync, fuer einfache Checks)
    pub connection: Mutex<ConnectionState>,
    /// Echte TCP-Verbindung (async Mutex, da await in send_and_receive)
    pub tcp: AsyncMutex<Option<ServerConnection>>,
    pub audio: Mutex<AudioState>,
    pub plugin_manager: Mutex<Option<PluginManager>>,
    /// Voice-Client (async Mutex, da start/stop async sind)
    pub voice: AsyncMutex<Option<VoiceClient>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            connection: Mutex::new(ConnectionState::default()),
            tcp: AsyncMutex::new(None),
            audio: Mutex::new(AudioState::default()),
            plugin_manager: Mutex::new(None),
            voice: AsyncMutex::new(None),
        }
    }
}

impl AppState {
    /// Erstellt einen neuen AppState mit initialisiertem PluginManager
    pub fn mit_plugins() -> Self {
        let manager = PluginManager::neu(ManagerKonfiguration::default());
        Self {
            connection: Mutex::new(ConnectionState::default()),
            tcp: AsyncMutex::new(None),
            audio: Mutex::new(AudioState::default()),
            plugin_manager: Mutex::new(Some(manager)),
            voice: AsyncMutex::new(None),
        }
    }
}
