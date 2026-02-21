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

// --- Chat-Datentypen (Phase 4) ---

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileInfo {
    pub id: String,
    pub filename: String,
    pub mime_type: String,
    pub size_bytes: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatMessage {
    pub id: String,
    pub channel_id: String,
    pub sender_id: String,
    pub sender_name: String,
    pub content: String,
    pub message_type: String,
    pub reply_to: Option<String>,
    pub file_info: Option<FileInfo>,
    pub created_at: String,
    pub edited_at: Option<String>,
}

// --- Chat-Commands (Phase 4, Stubs) ---

/// Sendet eine Text-Nachricht in einen Kanal
#[tauri::command]
pub async fn send_message(
    channel_id: String,
    content: String,
    reply_to: Option<String>,
) -> Result<ChatMessage, String> {
    debug!("Sende Nachricht in Kanal {}", channel_id);

    // TODO Phase 4: Echte Implementierung via speakeasy-chat
    Ok(ChatMessage {
        id: uuid_stub(),
        channel_id,
        sender_id: "self".to_string(),
        sender_name: "Du".to_string(),
        content,
        message_type: "text".to_string(),
        reply_to,
        file_info: None,
        created_at: chrono_now(),
        edited_at: None,
    })
}

/// Laedt die Nachrichten-History eines Kanals
#[tauri::command]
pub async fn get_message_history(
    channel_id: String,
    before: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<ChatMessage>, String> {
    debug!(
        "Lade Nachrichten-History fuer Kanal {} (before={:?}, limit={:?})",
        channel_id, before, limit
    );

    // TODO Phase 4: Echte Implementierung via speakeasy-chat
    Ok(vec![])
}

/// Editiert eine Nachricht
#[tauri::command]
pub async fn edit_message(
    message_id: String,
    content: String,
) -> Result<ChatMessage, String> {
    debug!("Editiere Nachricht {}", message_id);

    // TODO Phase 4: Echte Implementierung via speakeasy-chat
    Ok(ChatMessage {
        id: message_id,
        channel_id: String::new(),
        sender_id: "self".to_string(),
        sender_name: "Du".to_string(),
        content,
        message_type: "text".to_string(),
        reply_to: None,
        file_info: None,
        created_at: chrono_now(),
        edited_at: Some(chrono_now()),
    })
}

/// Loescht eine Nachricht (Soft-Delete)
#[tauri::command]
pub async fn delete_message(message_id: String) -> Result<(), String> {
    debug!("Loesche Nachricht {}", message_id);
    // TODO Phase 4: Echte Implementierung via speakeasy-chat
    Ok(())
}

/// Laedt eine Datei hoch und postet sie als Nachricht
#[tauri::command]
pub async fn upload_file(
    channel_id: String,
    filename: String,
    mime_type: String,
    data: Vec<u8>,
) -> Result<ChatMessage, String> {
    debug!(
        "Lade Datei '{}' ({} Bytes) in Kanal {} hoch",
        filename,
        data.len(),
        channel_id
    );

    // TODO Phase 4: Echte Implementierung via speakeasy-chat
    let file_id = uuid_stub();
    Ok(ChatMessage {
        id: uuid_stub(),
        channel_id,
        sender_id: "self".to_string(),
        sender_name: "Du".to_string(),
        content: format!("{}:{}", file_id, filename),
        message_type: "file".to_string(),
        reply_to: None,
        file_info: Some(FileInfo {
            id: file_id,
            filename,
            mime_type,
            size_bytes: data.len() as i64,
        }),
        created_at: chrono_now(),
        edited_at: None,
    })
}

/// Laedt eine Datei herunter
#[tauri::command]
pub async fn download_file(file_id: String) -> Result<Vec<u8>, String> {
    debug!("Lade Datei {} herunter", file_id);
    // TODO Phase 4: Echte Implementierung via speakeasy-chat
    Ok(vec![])
}

/// Listet Dateien in einem Kanal auf
#[tauri::command]
pub async fn list_files(channel_id: String) -> Result<Vec<FileInfo>, String> {
    debug!("Liste Dateien in Kanal {}", channel_id);
    // TODO Phase 4: Echte Implementierung via speakeasy-chat
    Ok(vec![])
}

// --- Hilfsfunktionen ---

fn uuid_stub() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    format!("stub-{:010}", nanos)
}

fn chrono_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Minimales ISO8601-Format ohne externe Abhaengigkeit
    let (y, mo, d, h, mi, s) = epoch_to_datetime(secs);
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, mo, d, h, mi, s)
}

fn epoch_to_datetime(secs: u64) -> (u64, u64, u64, u64, u64, u64) {
    let s = secs % 60;
    let m = (secs / 60) % 60;
    let h = (secs / 3600) % 24;
    let days = secs / 86400;
    // Vereinfacht: grobe Jahresberechnung (genuegt fuer Stubs)
    let y = 1970 + days / 365;
    let mo = ((days % 365) / 30) + 1;
    let d = (days % 30) + 1;
    (y, mo.min(12), d.min(31), h, m, s)
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
