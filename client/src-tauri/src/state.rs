use std::sync::Mutex;

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
}

/// Globaler Anwendungszustand (Mutex-gesichert fuer Thread-Sicherheit)
#[derive(Debug, Default)]
pub struct AppState {
    pub connection: Mutex<ConnectionState>,
    pub audio: Mutex<AudioState>,
}
