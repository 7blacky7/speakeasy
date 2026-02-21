use serde::{Deserialize, Serialize};
use tauri::State;
use tracing::{debug, info};

use crate::state::AppState;

// --- Datentypen ---

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub is_default: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AudioConfig {
    pub input_device_id: Option<String>,
    pub output_device_id: Option<String>,
    pub input_volume: f32,
    pub output_volume: f32,
    pub noise_suppression: bool,
    pub echo_cancellation: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ClientInfo {
    pub id: String,
    pub username: String,
    pub is_muted: bool,
    pub is_deafened: bool,
    pub is_self: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChannelInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub parent_id: Option<String>,
    pub clients: Vec<ClientInfo>,
    pub max_clients: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServerInfo {
    pub name: String,
    pub description: String,
    pub version: String,
    pub max_clients: u32,
    pub online_clients: u32,
    pub channels: Vec<ChannelInfo>,
}

// --- Commands ---

/// Verbindet sich mit einem Speakeasy-Server
#[tauri::command]
pub async fn connect_to_server(
    state: State<'_, AppState>,
    address: String,
    port: u16,
    username: String,
    password: Option<String>,
) -> Result<(), String> {
    info!(
        "Verbinde mit {}:{} als '{}' (Passwort: {})",
        address,
        port,
        username,
        if password.is_some() { "ja" } else { "nein" }
    );

    let mut conn = state.connection.lock().map_err(|e| e.to_string())?;
    conn.connected = true;
    conn.server_address = Some(address);
    conn.server_port = Some(port);
    conn.username = Some(username);

    // TODO Phase 3: Echte TCP/TLS-Verbindung via speakeasy-protocol
    Ok(())
}

/// Trennt die Verbindung zum Server
#[tauri::command]
pub async fn disconnect(state: State<'_, AppState>) -> Result<(), String> {
    info!("Verbindung wird getrennt");

    let mut conn = state.connection.lock().map_err(|e| e.to_string())?;
    conn.connected = false;
    conn.server_address = None;
    conn.server_port = None;
    conn.current_channel = None;

    // TODO Phase 3: Echte Trennung
    Ok(())
}

/// Tritt einem Kanal bei
#[tauri::command]
pub async fn join_channel(
    state: State<'_, AppState>,
    channel_id: String,
) -> Result<(), String> {
    debug!("Trete Kanal {} bei", channel_id);

    let mut conn = state.connection.lock().map_err(|e| e.to_string())?;
    if !conn.connected {
        return Err("Nicht mit einem Server verbunden".to_string());
    }
    conn.current_channel = Some(channel_id);

    // TODO Phase 3: Kanal-Beitritt via Protokoll + Audio-Stream starten
    Ok(())
}

/// Verlaesst den aktuellen Kanal
#[tauri::command]
pub async fn leave_channel(state: State<'_, AppState>) -> Result<(), String> {
    debug!("Verlasse aktuellen Kanal");

    let mut conn = state.connection.lock().map_err(|e| e.to_string())?;
    conn.current_channel = None;

    // TODO Phase 3: Kanal-Verlassen via Protokoll + Audio-Stream stoppen
    Ok(())
}

/// Gibt verfuegbare Audio-Geraete zurueck
#[tauri::command]
pub async fn get_audio_devices() -> Result<Vec<AudioDevice>, String> {
    debug!("Frage Audio-Geraete ab");

    // TODO Phase 3: Echte Geraete via speakeasy-audio / cpal
    Ok(vec![
        AudioDevice {
            id: "default-input".to_string(),
            name: "Standard-Mikrofon".to_string(),
            kind: "input".to_string(),
            is_default: true,
        },
        AudioDevice {
            id: "default-output".to_string(),
            name: "Standard-Lautsprecher".to_string(),
            kind: "output".to_string(),
            is_default: true,
        },
    ])
}

/// Setzt die Audio-Konfiguration
#[tauri::command]
pub async fn set_audio_config(config: AudioConfig) -> Result<(), String> {
    debug!(
        "Setze Audio-Konfiguration: input={:?}, output={:?}",
        config.input_device_id, config.output_device_id
    );

    // TODO Phase 3: Konfiguration an speakeasy-audio weitergeben
    Ok(())
}

/// Schaltet das Mikrofon stumm/wieder ein
#[tauri::command]
pub async fn toggle_mute(state: State<'_, AppState>) -> Result<bool, String> {
    let mut audio = state.audio.lock().map_err(|e| e.to_string())?;
    audio.muted = !audio.muted;
    let muted = audio.muted;
    info!("Mikrofon: {}", if muted { "stumm" } else { "aktiv" });

    // TODO Phase 3: Audio-Engine informieren
    Ok(muted)
}

/// Schaltet den Ton aus/ein (deaf)
#[tauri::command]
pub async fn toggle_deafen(state: State<'_, AppState>) -> Result<bool, String> {
    let mut audio = state.audio.lock().map_err(|e| e.to_string())?;
    audio.deafened = !audio.deafened;
    let deafened = audio.deafened;
    info!("Ton: {}", if deafened { "aus" } else { "ein" });

    // TODO Phase 3: Audio-Engine informieren
    Ok(deafened)
}

/// Gibt Server-Informationen zurueck
#[tauri::command]
pub async fn get_server_info(state: State<'_, AppState>) -> Result<ServerInfo, String> {
    let conn = state.connection.lock().map_err(|e| e.to_string())?;
    if !conn.connected {
        return Err("Nicht mit einem Server verbunden".to_string());
    }
    let username = conn.username.clone().unwrap_or_else(|| "Unbekannt".to_string());
    drop(conn);

    debug!("Frage Server-Info ab");

    // TODO Phase 3: Echte Daten vom Server holen
    Ok(ServerInfo {
        name: "Speakeasy Demo-Server".to_string(),
        description: "Lokaler Entwicklungsserver".to_string(),
        version: "0.1.0".to_string(),
        max_clients: 100,
        online_clients: 3,
        channels: vec![
            ChannelInfo {
                id: "1".to_string(),
                name: "Allgemein".to_string(),
                description: "Allgemeiner Sprachkanal".to_string(),
                parent_id: None,
                max_clients: 20,
                clients: vec![
                    ClientInfo {
                        id: "self".to_string(),
                        username: username.clone(),
                        is_muted: false,
                        is_deafened: false,
                        is_self: true,
                    },
                    ClientInfo {
                        id: "2".to_string(),
                        username: "Alice".to_string(),
                        is_muted: false,
                        is_deafened: false,
                        is_self: false,
                    },
                ],
            },
            ChannelInfo {
                id: "2".to_string(),
                name: "Gaming".to_string(),
                description: "Fuer Gaming-Sessions".to_string(),
                parent_id: None,
                max_clients: 10,
                clients: vec![ClientInfo {
                    id: "3".to_string(),
                    username: "Bob".to_string(),
                    is_muted: true,
                    is_deafened: false,
                    is_self: false,
                }],
            },
            ChannelInfo {
                id: "3".to_string(),
                name: "AFK".to_string(),
                description: "Abwesend".to_string(),
                parent_id: None,
                max_clients: 50,
                clients: vec![],
            },
        ],
    })
}
