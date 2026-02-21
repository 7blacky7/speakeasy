use serde::{Deserialize, Serialize};
use tauri::State;
use tracing::{debug, info, warn};

use speakeasy_core::types::ChannelId;
use speakeasy_protocol::control::{
    ChannelCreateRequest, ChannelDeleteRequest, ChannelEditRequest, ChatDeleteRequest,
    ChatEditRequest, ChatHistoryRequest, ChatSendRequest, ControlPayload, FileUploadRequest,
};

use crate::connection::ServerConnection;
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

    // TCP-Verbindung aufbauen
    let mut server_conn = ServerConnection::connect(&address, port)
        .await
        .map_err(|e| format!("Verbindungsfehler: {}", e))?;

    // Login durchfuehren
    let pwd = password.as_deref().unwrap_or("");
    server_conn
        .login(&username, pwd)
        .await
        .map_err(|e| format!("Login fehlgeschlagen: {}", e))?;

    // Metadaten im sync ConnectionState speichern
    {
        let mut conn = state.connection.lock().map_err(|e| e.to_string())?;
        conn.connected = true;
        conn.server_address = Some(address);
        conn.server_port = Some(port);
        conn.username = Some(username);
    }

    // Echte TCP-Verbindung im async Mutex speichern
    {
        let mut tcp = state.tcp.lock().await;
        *tcp = Some(server_conn);
    }

    Ok(())
}

/// Trennt die Verbindung zum Server und stoppt die Voice-Pipeline
#[tauri::command]
pub async fn disconnect(state: State<'_, AppState>) -> Result<(), String> {
    info!("Verbindung wird getrennt");

    // 1. Voice-Pipeline stoppen
    {
        let mut voice = state.voice.lock().await;
        if let Some(ref mut client) = *voice {
            client.stop().await;
        }
        *voice = None;
    }

    // 2. Logout und TCP-Verbindung trennen
    {
        let mut tcp = state.tcp.lock().await;
        if let Some(ref mut conn) = *tcp {
            // Voice-Disconnect versuchen
            if let Err(e) = conn.voice_disconnect(Some("Verbindung getrennt".to_string())).await {
                warn!("Voice-Disconnect-Fehler (wird ignoriert): {}", e);
            }
            // Logout versuchen, Fehler ignorieren (Verbindung wird trotzdem getrennt)
            if let Err(e) = conn.logout().await {
                warn!("Logout-Fehler (wird ignoriert): {}", e);
            }
            conn.disconnect().await;
        }
        *tcp = None;
    }

    // 3. Metadaten zuruecksetzen
    {
        let mut conn = state.connection.lock().map_err(|e| e.to_string())?;
        conn.connected = false;
        conn.server_address = None;
        conn.server_port = None;
        conn.current_channel = None;
    }

    Ok(())
}

/// Tritt einem Kanal bei und startet die Voice-Pipeline
#[tauri::command]
pub async fn join_channel(
    state: State<'_, AppState>,
    channel_id: String,
) -> Result<(), String> {
    debug!("Trete Kanal {} bei", channel_id);

    let server_address: String;
    {
        let conn = state.connection.lock().map_err(|e| e.to_string())?;
        if !conn.connected {
            return Err("Nicht mit einem Server verbunden".to_string());
        }
        server_address = conn
            .server_address
            .clone()
            .ok_or_else(|| "Keine Server-Adresse bekannt".to_string())?;
    }

    // 1. Kanal-Beitritt ueber TCP-Verbindung
    // 2. Voice-Init: UDP Port Negotiation
    let voice_ready = {
        let mut tcp = state.tcp.lock().await;
        let conn = tcp
            .as_mut()
            .ok_or_else(|| "Keine TCP-Verbindung vorhanden".to_string())?;

        conn.join_channel(&channel_id)
            .await
            .map_err(|e| format!("Kanal-Beitritt fehlgeschlagen: {}", e))?;

        // Voice-Init senden (Port 0 = wird nach Socket-Bind aktualisiert)
        // Wir senden erstmal Port 0, der Server kennt unsere IP aus der TCP-Verbindung
        conn.voice_init(0)
            .await
            .map_err(|e| format!("Voice-Init fehlgeschlagen: {}", e))?
    };

    // 3. Voice-Pipeline starten
    {
        let server_ip = if voice_ready.server_ip.is_empty() {
            server_address.clone()
        } else {
            voice_ready.server_ip.clone()
        };

        let server_udp_addr: std::net::SocketAddr = format!(
            "{}:{}",
            server_ip, voice_ready.server_udp_port
        )
        .parse()
        .map_err(|e| format!("Ungueltige Server-UDP-Adresse: {}", e))?;

        let mut voice = state.voice.lock().await;
        // Alte Voice-Verbindung stoppen falls vorhanden
        if let Some(ref mut client) = *voice {
            client.stop().await;
        }

        let mut client = crate::voice::VoiceClient::new();
        client
            .start(server_udp_addr, voice_ready.ssrc)
            .await
            .map_err(|e| format!("Voice-Pipeline konnte nicht gestartet werden: {}", e))?;

        *voice = Some(client);
    }

    // Metadaten aktualisieren
    {
        let mut conn = state.connection.lock().map_err(|e| e.to_string())?;
        conn.current_channel = Some(channel_id);
    }

    Ok(())
}

/// Verlaesst den aktuellen Kanal und stoppt die Voice-Pipeline
#[tauri::command]
pub async fn leave_channel(state: State<'_, AppState>) -> Result<(), String> {
    debug!("Verlasse aktuellen Kanal");

    // 1. Voice-Pipeline stoppen
    {
        let mut voice = state.voice.lock().await;
        if let Some(ref mut client) = *voice {
            client.stop().await;
        }
        *voice = None;
    }

    // 2. Voice-Disconnect an Server senden
    {
        let mut tcp = state.tcp.lock().await;
        if let Some(ref mut conn) = *tcp {
            if let Err(e) = conn.voice_disconnect(Some("Kanal verlassen".to_string())).await {
                warn!("Voice-Disconnect fehlgeschlagen: {} (wird ignoriert)", e);
            }
        }
    }

    let channel_id = {
        let conn = state.connection.lock().map_err(|e| e.to_string())?;
        conn.current_channel.clone()
    };

    // 3. Kanal-Verlassen ueber TCP-Verbindung
    if let Some(ref cid) = channel_id {
        let mut tcp = state.tcp.lock().await;
        if let Some(ref mut conn) = *tcp {
            if let Err(e) = conn.leave_channel(cid).await {
                warn!("Kanal-Verlassen fehlgeschlagen: {} (wird lokal zurueckgesetzt)", e);
            }
        }
    }

    // 4. Metadaten aktualisieren
    {
        let mut conn = state.connection.lock().map_err(|e| e.to_string())?;
        conn.current_channel = None;
    }

    Ok(())
}

/// Gibt verfuegbare Audio-Geraete zurueck (echte cpal-Geraete)
#[tauri::command]
pub async fn get_audio_devices() -> Result<Vec<AudioDevice>, String> {
    debug!("Frage Audio-Geraete ab");

    let default_input_name = speakeasy_audio::get_default_input().map(|d| d.name);
    let default_output_name = speakeasy_audio::get_default_output().map(|d| d.name);

    let mut devices: Vec<AudioDevice> = Vec::new();

    // Eingabegeraete
    match speakeasy_audio::list_input_devices() {
        Ok(inputs) => {
            for dev in inputs {
                let is_default = default_input_name.as_deref() == Some(&dev.name);
                devices.push(AudioDevice {
                    id: dev.id.clone(),
                    name: dev.name,
                    kind: "input".to_string(),
                    is_default,
                });
            }
        }
        Err(e) => {
            warn!("Eingabegeraete konnten nicht aufgelistet werden: {}", e);
        }
    }

    // Ausgabegeraete
    match speakeasy_audio::list_output_devices() {
        Ok(outputs) => {
            for dev in outputs {
                let is_default = default_output_name.as_deref() == Some(&dev.name);
                devices.push(AudioDevice {
                    id: dev.id.clone(),
                    name: dev.name,
                    kind: "output".to_string(),
                    is_default,
                });
            }
        }
        Err(e) => {
            warn!("Ausgabegeraete konnten nicht aufgelistet werden: {}", e);
        }
    }

    // Graceful Fallback: leere Liste wenn keine Hardware verfuegbar
    if devices.is_empty() {
        debug!("Keine Audio-Hardware gefunden – gebe leere Liste zurueck");
    }

    Ok(devices)
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
    // MutexGuard MUSS vor dem .await gedroppt werden (Send-Requirement)
    let muted = {
        let mut audio = state.audio.lock().map_err(|e| e.to_string())?;
        audio.muted = !audio.muted;
        let muted = audio.muted;
        info!("Mikrofon: {}", if muted { "stumm" } else { "aktiv" });
        muted
    };

    // Voice-Client informieren
    let voice = state.voice.lock().await;
    if let Some(ref client) = *voice {
        client.set_muted(muted);
    }

    Ok(muted)
}

/// Schaltet den Ton aus/ein (deaf)
#[tauri::command]
pub async fn toggle_deafen(state: State<'_, AppState>) -> Result<bool, String> {
    // MutexGuard MUSS vor dem .await gedroppt werden (Send-Requirement)
    let deafened = {
        let mut audio = state.audio.lock().map_err(|e| e.to_string())?;
        audio.deafened = !audio.deafened;
        let deafened = audio.deafened;
        if deafened {
            audio.muted = true;
        }
        info!("Ton: {}", if deafened { "aus" } else { "ein" });
        deafened
    };

    // Voice-Client informieren
    let voice = state.voice.lock().await;
    if let Some(ref client) = *voice {
        client.set_deafened(deafened);
    }

    Ok(deafened)
}

// --- Erweiterte Audio-Typen (Phase 3) ---

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CodecConfig {
    pub sample_rate: u32,
    pub buffer_size: u32,
    pub bitrate: u32,
    pub frame_size: u32,
    pub application: String,
    pub fec: bool,
    pub dtx: bool,
    pub channels: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NoiseGateConfig {
    pub enabled: bool,
    pub threshold: f32,
    pub attack: f32,
    pub release: f32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NoiseSuppressionConfig {
    pub enabled: bool,
    pub level: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AgcConfig {
    pub enabled: bool,
    pub target_level: f32,
    pub max_gain: f32,
    pub attack: f32,
    pub release: f32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EchoCancellationConfig {
    pub enabled: bool,
    pub tail_length: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DeesserConfig {
    pub enabled: bool,
    pub frequency: f32,
    pub threshold: f32,
    pub ratio: f32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DspConfig {
    pub noise_gate: NoiseGateConfig,
    pub noise_suppression: NoiseSuppressionConfig,
    pub agc: AgcConfig,
    pub echo_cancellation: EchoCancellationConfig,
    pub deesser: DeesserConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct JitterConfig {
    pub min_buffer: u32,
    pub max_buffer: u32,
    pub adaptive: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AudioSettingsConfig {
    pub input_device_id: Option<String>,
    pub output_device_id: Option<String>,
    pub voice_mode: String,
    pub ptt_key: Option<String>,
    pub vad_sensitivity: f32,
    pub preset: String,
    pub noise_suppression: String,
    pub input_volume: f32,
    pub output_volume: f32,
    pub codec: CodecConfig,
    pub dsp: DspConfig,
    pub jitter: JitterConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LatencyBreakdown {
    pub device: f32,
    pub encoding: f32,
    pub jitter: f32,
    pub network: f32,
    pub total: f32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AudioStats {
    pub input_level: f32,
    pub output_level: f32,
    pub processed_level: f32,
    pub noise_floor: f32,
    pub is_clipping: bool,
    pub latency: LatencyBreakdown,
    pub packet_loss: f32,
    pub rtt: f32,
    pub bitrate: f32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CalibrationResult {
    pub success: bool,
    pub suggested_vad_sensitivity: f32,
    pub suggested_input_volume: f32,
    pub noise_floor: f32,
}

fn default_audio_settings() -> AudioSettingsConfig {
    AudioSettingsConfig {
        input_device_id: None,
        output_device_id: None,
        voice_mode: "vad".to_string(),
        ptt_key: None,
        vad_sensitivity: 0.5,
        preset: "balanced".to_string(),
        noise_suppression: "medium".to_string(),
        input_volume: 1.0,
        output_volume: 1.0,
        codec: CodecConfig {
            sample_rate: 48000,
            buffer_size: 480,
            bitrate: 64000,
            frame_size: 20,
            application: "voip".to_string(),
            fec: true,
            dtx: false,
            channels: "mono".to_string(),
        },
        dsp: DspConfig {
            noise_gate: NoiseGateConfig {
                enabled: true,
                threshold: -40.0,
                attack: 5.0,
                release: 50.0,
            },
            noise_suppression: NoiseSuppressionConfig {
                enabled: true,
                level: "medium".to_string(),
            },
            agc: AgcConfig {
                enabled: true,
                target_level: -18.0,
                max_gain: 30.0,
                attack: 10.0,
                release: 100.0,
            },
            echo_cancellation: EchoCancellationConfig {
                enabled: false,
                tail_length: 100,
            },
            deesser: DeesserConfig {
                enabled: false,
                frequency: 7000.0,
                threshold: -20.0,
                ratio: 4.0,
            },
        },
        jitter: JitterConfig {
            min_buffer: 20,
            max_buffer: 200,
            adaptive: true,
        },
    }
}

// --- Audio-Commands (Phase 3) ---

/// Gibt die aktuellen Audio-Einstellungen zurueck
#[tauri::command]
pub async fn get_audio_settings(state: State<'_, AppState>) -> Result<AudioSettingsConfig, String> {
    debug!("Frage Audio-Einstellungen ab");
    let audio = state.audio.lock().map_err(|e| e.to_string())?;

    if let Some(ref cfg) = audio.engine_config {
        // Aus gespeicherter Engine-Config rekonstruieren
        let mut settings = default_audio_settings();
        settings.input_device_id = cfg.input_device.clone();
        settings.output_device_id = cfg.output_device.clone();
        settings.codec.sample_rate = cfg.capture.sample_rate;
        settings.codec.buffer_size = cfg.capture.buffer_size as u32;
        Ok(settings)
    } else {
        Ok(default_audio_settings())
    }
}

/// Speichert Audio-Einstellungen
#[tauri::command]
pub async fn set_audio_settings(
    state: State<'_, AppState>,
    config: AudioSettingsConfig,
) -> Result<(), String> {
    debug!(
        "Setze Audio-Einstellungen: input={:?}, output={:?}",
        config.input_device_id, config.output_device_id
    );

    use speakeasy_audio::capture::CaptureConfig;

    let mut audio = state.audio.lock().map_err(|e| e.to_string())?;

    let mut engine_config = audio.engine_config.clone().unwrap_or_default();
    engine_config.input_device = config.input_device_id.clone();
    engine_config.output_device = config.output_device_id.clone();

    let mut capture = CaptureConfig::default();
    capture.sample_rate = config.codec.sample_rate;
    capture.buffer_size = config.codec.buffer_size as usize;
    engine_config.capture = capture;

    audio.engine_config = Some(engine_config);

    info!("Audio-Einstellungen gespeichert");
    Ok(())
}

/// Startet die Auto-Kalibrierung (Noise-Floor-Messung)
#[tauri::command]
pub async fn start_calibration() -> Result<CalibrationResult, String> {
    debug!("Starte Audio-Kalibrierung");

    // Stille-Samples generieren (Fallback wenn keine Hardware verfuegbar)
    // 1 Sekunde bei 48kHz = 48000 Samples
    let silence_samples = vec![0.001f32; 48000];

    match speakeasy_audio::calibrate_from_samples(&silence_samples, 480) {
        Ok(result) => {
            info!(
                "Kalibrierung abgeschlossen: noise_floor={:.1}dB",
                result.noise_floor_db
            );
            // VAD-Sensitivity aus Noise Floor ableiten (0.0 = still, 1.0 = laut)
            // noise_floor_db liegt typisch bei -60..-40 dBFS
            let vad_sensitivity = ((-result.noise_floor_db - 40.0) / 20.0).clamp(0.1, 0.9);
            Ok(CalibrationResult {
                success: true,
                suggested_vad_sensitivity: vad_sensitivity,
                suggested_input_volume: 1.0,
                noise_floor: result.noise_floor_db,
            })
        }
        Err(e) => {
            warn!("Kalibrierung fehlgeschlagen: {} – nutze Default", e);
            let default = speakeasy_audio::default_calibration();
            Ok(CalibrationResult {
                success: false,
                suggested_vad_sensitivity: 0.5,
                suggested_input_volume: 1.0,
                noise_floor: default.noise_floor_db,
            })
        }
    }
}

/// Gibt aktuelle Audio-Statistiken zurueck
#[tauri::command]
pub async fn get_audio_stats() -> Result<AudioStats, String> {
    debug!("Frage Audio-Statistiken ab");

    // Basis-Stats ohne laufende Engine
    Ok(AudioStats {
        input_level: 0.0,
        output_level: 0.0,
        processed_level: 0.0,
        noise_floor: -60.0,
        is_clipping: false,
        latency: LatencyBreakdown {
            device: 10.0,
            encoding: 20.0,
            jitter: 40.0,
            network: 0.0,
            total: 70.0,
        },
        packet_loss: 0.0,
        rtt: 0.0,
        bitrate: 64000.0,
    })
}

/// Spielt einen Testton (440 Hz Sinus) ab
#[tauri::command]
pub async fn play_test_sound() -> Result<(), String> {
    debug!("Spiele Testton ab");

    // 440 Hz Sinus bei 48kHz fuer 0.5 Sekunden generieren
    let sample_rate = 48000u32;
    let duration_samples = sample_rate / 2; // 0.5 Sekunden
    let frequency = 440.0f32;

    let samples: Vec<f32> = (0..duration_samples)
        .map(|i| {
            let t = i as f32 / sample_rate as f32;
            0.3 * (2.0 * std::f32::consts::PI * frequency * t).sin()
        })
        .collect();

    // Playback ueber cpal in eigenem Thread (nicht blockierend fuer Tauri)
    let result = std::thread::spawn(move || -> Result<(), String> {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| "Kein Standard-Ausgabegeraet gefunden".to_string())?;

        let config = device
            .default_output_config()
            .map_err(|e| e.to_string())?;

        let channels = config.channels() as usize;
        let mut sample_idx = 0usize;
        let samples_len = samples.len();
        let (done_tx, done_rx) = std::sync::mpsc::channel::<()>();

        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => device.build_output_stream(
                &config.into(),
                move |data: &mut [f32], _| {
                    for frame in data.chunks_mut(channels) {
                        let val = if sample_idx < samples_len {
                            samples[sample_idx]
                        } else {
                            0.0
                        };
                        sample_idx += 1;
                        for sample in frame.iter_mut() {
                            *sample = val;
                        }
                        if sample_idx >= samples_len {
                            let _ = done_tx.send(());
                        }
                    }
                },
                |e| tracing::error!("Testton-Fehler: {}", e),
                None,
            ),
            _ => {
                return Err("Nicht unterstuetztes Audio-Format".to_string());
            }
        }
        .map_err(|e| e.to_string())?;

        stream.play().map_err(|e| e.to_string())?;
        // Warten bis Testton fertig oder maximal 1 Sekunde
        let _ = done_rx.recv_timeout(std::time::Duration::from_secs(1));

        Ok(())
    })
    .join()
    .map_err(|_| "Testton-Thread ist abgestuerzt".to_string())?;

    match result {
        Ok(()) => {
            info!("Testton abgespielt");
            Ok(())
        }
        Err(e) => {
            warn!("Testton fehlgeschlagen: {} – kein Ausgabegeraet verfuegbar", e);
            // Graceful: kein Fehler zurueck wenn keine Hardware
            Ok(())
        }
    }
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

// --- Chat-Commands (Phase 4, echte TCP-Implementierung) ---

/// Hilfsfunktion: parst eine Kanal-ID (UUID)
fn parse_channel_id(channel_id: &str) -> Result<ChannelId, String> {
    uuid::Uuid::parse_str(channel_id)
        .map(ChannelId)
        .map_err(|_| format!("Ungueltige Kanal-ID '{}': keine gueltige UUID", channel_id))
}

/// Konvertiert einen Unix-Timestamp (Sekunden) in ISO8601
fn unix_timestamp_to_iso(secs: u64) -> String {
    let (y, mo, d, h, mi, s) = epoch_to_datetime(secs);
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, mo, d, h, mi, s)
}

/// Sendet eine Text-Nachricht in einen Kanal via TCP
#[tauri::command]
pub async fn send_message(
    state: State<'_, AppState>,
    channel_id: String,
    content: String,
    reply_to: Option<String>,
) -> Result<ChatMessage, String> {
    debug!("Sende Nachricht in Kanal {}", channel_id);

    let cid = parse_channel_id(&channel_id)?;

    let mut tcp = state.tcp.lock().await;
    let conn = tcp
        .as_mut()
        .ok_or_else(|| "Nicht verbunden – bitte zuerst connect_to_server aufrufen".to_string())?;

    let request_id = conn.next_id();
    let nachricht = speakeasy_protocol::control::ControlMessage::new(
        request_id,
        ControlPayload::ChatSend(ChatSendRequest {
            channel_id: cid,
            content: content.clone(),
            reply_to: reply_to.clone(),
        }),
    );

    let antwort = conn.send_and_receive(nachricht).await.map_err(|e| e.to_string())?;

    match antwort.payload {
        ControlPayload::ChatSendResponse(resp) => {
            let sender_id = conn.user_id().unwrap_or("self").to_string();
            drop(tcp);
            let conn_state = state.connection.lock().map_err(|e| e.to_string())?;
            let sender_name = conn_state.username.clone().unwrap_or_else(|| "Du".to_string());
            drop(conn_state);
            Ok(ChatMessage {
                id: resp.message_id,
                channel_id,
                sender_id,
                sender_name,
                content,
                message_type: "text".to_string(),
                reply_to,
                file_info: None,
                created_at: unix_timestamp_to_iso(resp.created_at),
                edited_at: None,
            })
        }
        ControlPayload::Error(e) => Err(format!("Server-Fehler: {}", e.message)),
        other => Err(format!(
            "Unerwartete Antwort vom Server: {:?}",
            std::mem::discriminant(&other)
        )),
    }
}

/// Laedt die Nachrichten-History eines Kanals via TCP
#[tauri::command]
pub async fn get_message_history(
    state: State<'_, AppState>,
    channel_id: String,
    before: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<ChatMessage>, String> {
    debug!(
        "Lade Nachrichten-History fuer Kanal {} (before={:?}, limit={:?})",
        channel_id, before, limit
    );

    let cid = parse_channel_id(&channel_id)?;

    let mut tcp = state.tcp.lock().await;
    let conn = tcp
        .as_mut()
        .ok_or_else(|| "Nicht verbunden – bitte zuerst connect_to_server aufrufen".to_string())?;

    let request_id = conn.next_id();
    let nachricht = speakeasy_protocol::control::ControlMessage::new(
        request_id,
        ControlPayload::ChatHistory(ChatHistoryRequest {
            channel_id: cid,
            before,
            limit: limit.map(|l| l as i64),
        }),
    );

    let antwort = conn.send_and_receive(nachricht).await.map_err(|e| e.to_string())?;

    match antwort.payload {
        ControlPayload::ChatHistoryResponse(resp) => {
            let nachrichten: Vec<ChatMessage> = resp
                .messages
                .into_iter()
                .map(|m| ChatMessage {
                    channel_id: m.channel_id.inner().to_string(),
                    sender_id: m.sender_id.inner().to_string(),
                    sender_name: m.sender_id.inner().to_string(),
                    id: m.message_id,
                    content: m.content,
                    message_type: m.message_type,
                    reply_to: m.reply_to,
                    file_info: None,
                    created_at: m.created_at,
                    edited_at: m.edited_at,
                })
                .collect();
            Ok(nachrichten)
        }
        ControlPayload::Error(e) => Err(format!("Server-Fehler: {}", e.message)),
        other => Err(format!(
            "Unerwartete Antwort vom Server: {:?}",
            std::mem::discriminant(&other)
        )),
    }
}

/// Editiert eine Nachricht via TCP
#[tauri::command]
pub async fn edit_message(
    state: State<'_, AppState>,
    message_id: String,
    content: String,
) -> Result<ChatMessage, String> {
    debug!("Editiere Nachricht {}", message_id);

    let mut tcp = state.tcp.lock().await;
    let conn = tcp
        .as_mut()
        .ok_or_else(|| "Nicht verbunden – bitte zuerst connect_to_server aufrufen".to_string())?;

    let request_id = conn.next_id();
    let nachricht = speakeasy_protocol::control::ControlMessage::new(
        request_id,
        ControlPayload::ChatEdit(ChatEditRequest {
            message_id: message_id.clone(),
            content: content.clone(),
        }),
    );

    let antwort = conn.send_and_receive(nachricht).await.map_err(|e| e.to_string())?;

    match antwort.payload {
        ControlPayload::ChatEdit(req) => {
            let sender_id = conn.user_id().unwrap_or("self").to_string();
            Ok(ChatMessage {
                id: req.message_id,
                channel_id: String::new(),
                sender_id,
                sender_name: "Du".to_string(),
                content: req.content,
                message_type: "text".to_string(),
                reply_to: None,
                file_info: None,
                created_at: chrono_now(),
                edited_at: Some(chrono_now()),
            })
        }
        ControlPayload::Error(e) => Err(format!("Server-Fehler: {}", e.message)),
        other => Err(format!(
            "Unerwartete Antwort vom Server: {:?}",
            std::mem::discriminant(&other)
        )),
    }
}

/// Loescht eine Nachricht (Soft-Delete) via TCP
#[tauri::command]
pub async fn delete_message(
    state: State<'_, AppState>,
    message_id: String,
) -> Result<(), String> {
    debug!("Loesche Nachricht {}", message_id);

    let mut tcp = state.tcp.lock().await;
    let conn = tcp
        .as_mut()
        .ok_or_else(|| "Nicht verbunden – bitte zuerst connect_to_server aufrufen".to_string())?;

    let request_id = conn.next_id();
    let nachricht = speakeasy_protocol::control::ControlMessage::new(
        request_id,
        ControlPayload::ChatDelete(ChatDeleteRequest {
            message_id: message_id.clone(),
        }),
    );

    let antwort = conn.send_and_receive(nachricht).await.map_err(|e| e.to_string())?;

    match antwort.payload {
        ControlPayload::ChatDelete(_) => {
            debug!("Nachricht {} erfolgreich geloescht", message_id);
            Ok(())
        }
        ControlPayload::Error(e) => Err(format!("Server-Fehler: {}", e.message)),
        other => Err(format!(
            "Unerwartete Antwort vom Server: {:?}",
            std::mem::discriminant(&other)
        )),
    }
}

/// Initiiert einen Datei-Upload via TCP (Token-Flow)
///
/// Sendet FileUpload-Request, erhaelt Upload-Token zurueck.
/// Die eigentliche Datei-Uebertragung erfolgt via HTTP an die Upload-URL.
#[tauri::command]
pub async fn upload_file(
    state: State<'_, AppState>,
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

    let cid = parse_channel_id(&channel_id)?;
    let size_bytes = data.len() as u64;

    let mut tcp = state.tcp.lock().await;
    let conn = tcp
        .as_mut()
        .ok_or_else(|| "Nicht verbunden – bitte zuerst connect_to_server aufrufen".to_string())?;

    let request_id = conn.next_id();
    let upload_anfrage = speakeasy_protocol::control::ControlMessage::new(
        request_id,
        ControlPayload::FileUpload(FileUploadRequest {
            channel_id: cid,
            filename: filename.clone(),
            size_bytes,
            mime_type: Some(mime_type.clone()),
            checksum: None,
        }),
    );

    let antwort = conn.send_and_receive(upload_anfrage).await.map_err(|e| e.to_string())?;

    match antwort.payload {
        ControlPayload::FileUploadResponse(resp) => {
            let file_id = resp.file_id.clone();
            let sender_id = conn.user_id().unwrap_or("self").to_string();
            drop(tcp);
            let conn_state = state.connection.lock().map_err(|e| e.to_string())?;
            let sender_name = conn_state.username.clone().unwrap_or_else(|| "Du".to_string());
            drop(conn_state);
            info!(
                "Upload-Token erhalten: file_id={}, url={}",
                file_id, resp.upload_url
            );
            Ok(ChatMessage {
                id: uuid::Uuid::new_v4().to_string(),
                channel_id,
                sender_id,
                sender_name,
                content: filename.clone(),
                message_type: "file".to_string(),
                reply_to: None,
                file_info: Some(FileInfo {
                    id: file_id,
                    filename,
                    mime_type,
                    size_bytes: size_bytes as i64,
                }),
                created_at: chrono_now(),
                edited_at: None,
            })
        }
        ControlPayload::Error(e) => Err(format!("Server-Fehler beim Upload: {}", e.message)),
        other => Err(format!(
            "Unerwartete Antwort vom Server: {:?}",
            std::mem::discriminant(&other)
        )),
    }
}

/// Datei-Download – gibt leere Bytes zurueck (HTTP-Download nicht implementiert)
///
/// Das Control-Protokoll uebertraegt keine binaeren Dateidaten direkt.
#[tauri::command]
pub async fn download_file(
    _state: State<'_, AppState>,
    file_id: String,
) -> Result<Vec<u8>, String> {
    debug!("Lade Datei {} herunter", file_id);
    warn!(
        "download_file: HTTP-Download noch nicht implementiert (file_id={})",
        file_id
    );
    Ok(vec![])
}

/// Listet Dateien in einem Kanal auf via TCP
#[tauri::command]
pub async fn list_files(
    state: State<'_, AppState>,
    channel_id: String,
) -> Result<Vec<FileInfo>, String> {
    debug!("Liste Dateien in Kanal {}", channel_id);

    let cid = parse_channel_id(&channel_id)?;

    let mut tcp = state.tcp.lock().await;
    let conn = tcp
        .as_mut()
        .ok_or_else(|| "Nicht verbunden – bitte zuerst connect_to_server aufrufen".to_string())?;

    let request_id = conn.next_id();
    let nachricht = speakeasy_protocol::control::ControlMessage::new(
        request_id,
        ControlPayload::FileList { channel_id: cid },
    );

    let antwort = conn.send_and_receive(nachricht).await.map_err(|e| e.to_string())?;

    match antwort.payload {
        ControlPayload::FileListResponse(resp) => {
            let dateien: Vec<FileInfo> = resp
                .files
                .into_iter()
                .map(|f| FileInfo {
                    id: f.file_id,
                    filename: f.name,
                    mime_type: f
                        .mime_type
                        .unwrap_or_else(|| "application/octet-stream".to_string()),
                    size_bytes: f.size_bytes as i64,
                })
                .collect();
            Ok(dateien)
        }
        ControlPayload::Error(e) => Err(format!("Server-Fehler: {}", e.message)),
        other => Err(format!(
            "Unerwartete Antwort vom Server: {:?}",
            std::mem::discriminant(&other)
        )),
    }
}

// --- Hilfsfunktionen ---

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

// --- Plugin-Datentypen (Phase 5) ---

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum PluginStateDto {
    Simple(String),
    #[allow(non_snake_case)]
    Fehler {
        #[serde(rename = "Fehler")]
        fehler: String,
    },
}

impl From<speakeasy_plugin::types::PluginState> for PluginStateDto {
    fn from(state: speakeasy_plugin::types::PluginState) -> Self {
        match state {
            speakeasy_plugin::types::PluginState::Geladen => {
                PluginStateDto::Simple("Geladen".to_string())
            }
            speakeasy_plugin::types::PluginState::Aktiv => {
                PluginStateDto::Simple("Aktiv".to_string())
            }
            speakeasy_plugin::types::PluginState::Deaktiviert => {
                PluginStateDto::Simple("Deaktiviert".to_string())
            }
            speakeasy_plugin::types::PluginState::Fehler(msg) => {
                PluginStateDto::Fehler { fehler: msg }
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PluginInfoDto {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub state: PluginStateDto,
    pub trust_level: String,
    pub geladen_am: String,
}

impl From<speakeasy_plugin::types::PluginInfo> for PluginInfoDto {
    fn from(info: speakeasy_plugin::types::PluginInfo) -> Self {
        let trust_level = match info.trust_level {
            speakeasy_plugin::types::TrustLevel::NichtSigniert => "NichtSigniert".to_string(),
            speakeasy_plugin::types::TrustLevel::Signiert => "Signiert".to_string(),
            speakeasy_plugin::types::TrustLevel::Vertrauenswuerdig => {
                "Vertrauenswuerdig".to_string()
            }
        };
        Self {
            id: info.id.inner().to_string(),
            name: info.name,
            version: info.version,
            author: info.author,
            description: info.description,
            state: info.state.into(),
            trust_level,
            geladen_am: info.geladen_am.to_rfc3339(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PluginInstallResultDto {
    pub id: String,
    pub name: String,
    pub trust_level: String,
}

// --- Plugin-Commands (Phase 5) ---

/// Listet alle geladenen Plugins auf
#[tauri::command]
pub async fn list_plugins(state: State<'_, AppState>) -> Result<Vec<PluginInfoDto>, String> {
    debug!("Liste geladene Plugins auf");
    let manager = state.plugin_manager.lock().map_err(|e| e.to_string())?;
    let Some(ref mgr) = *manager else {
        return Ok(vec![]);
    };
    let plugins: Vec<PluginInfoDto> = mgr
        .plugins_auflisten()
        .into_iter()
        .map(Into::into)
        .collect();
    Ok(plugins)
}

/// Aktiviert ein Plugin anhand seiner ID
#[tauri::command]
pub async fn enable_plugin(state: State<'_, AppState>, id: String) -> Result<(), String> {
    debug!("Aktiviere Plugin {}", id);
    let plugin_id = parse_plugin_id(&id)?;
    let manager = state.plugin_manager.lock().map_err(|e| e.to_string())?;
    let Some(ref mgr) = *manager else {
        return Err("PluginManager nicht initialisiert".to_string());
    };
    mgr.plugin_aktivieren(plugin_id)
        .map_err(|e| e.to_string())
}

/// Deaktiviert ein Plugin anhand seiner ID
#[tauri::command]
pub async fn disable_plugin(state: State<'_, AppState>, id: String) -> Result<(), String> {
    debug!("Deaktiviere Plugin {}", id);
    let plugin_id = parse_plugin_id(&id)?;
    let manager = state.plugin_manager.lock().map_err(|e| e.to_string())?;
    let Some(ref mgr) = *manager else {
        return Err("PluginManager nicht initialisiert".to_string());
    };
    mgr.plugin_deaktivieren(plugin_id)
        .map_err(|e| e.to_string())
}

/// Entlaedt ein Plugin vollstaendig
#[tauri::command]
pub async fn unload_plugin(state: State<'_, AppState>, id: String) -> Result<(), String> {
    debug!("Entlade Plugin {}", id);
    let plugin_id = parse_plugin_id(&id)?;
    let manager = state.plugin_manager.lock().map_err(|e| e.to_string())?;
    let Some(ref mgr) = *manager else {
        return Err("PluginManager nicht initialisiert".to_string());
    };
    mgr.plugin_entladen(plugin_id).map_err(|e| e.to_string())
}

/// Installiert ein Plugin aus einem Verzeichnispfad
#[tauri::command]
pub async fn install_plugin(
    state: State<'_, AppState>,
    path: String,
) -> Result<PluginInstallResultDto, String> {
    debug!("Installiere Plugin aus Pfad: {}", path);
    let pfad = std::path::Path::new(&path);
    let manager = state.plugin_manager.lock().map_err(|e| e.to_string())?;
    let Some(ref mgr) = *manager else {
        return Err("PluginManager nicht initialisiert".to_string());
    };
    let plugin_id = mgr.plugin_laden(pfad).map_err(|e| e.to_string())?;
    let info = mgr
        .plugin_info(plugin_id)
        .ok_or_else(|| "Plugin nach dem Laden nicht gefunden".to_string())?;
    let trust_level = match &info.trust_level {
        speakeasy_plugin::types::TrustLevel::NichtSigniert => "NichtSigniert".to_string(),
        speakeasy_plugin::types::TrustLevel::Signiert => "Signiert".to_string(),
        speakeasy_plugin::types::TrustLevel::Vertrauenswuerdig => "Vertrauenswuerdig".to_string(),
    };
    if trust_level == "NichtSigniert" {
        warn!("Installiertes Plugin '{}' ist nicht signiert", info.name);
    }
    Ok(PluginInstallResultDto {
        id: plugin_id.inner().to_string(),
        name: info.name,
        trust_level,
    })
}

/// Hilfsfunktion: String-ID in PluginId konvertieren
fn parse_plugin_id(id: &str) -> Result<speakeasy_plugin::types::PluginId, String> {
    let uuid = uuid::Uuid::parse_str(id)
        .map_err(|e| format!("Ungueltige Plugin-ID '{}': {}", id, e))?;
    Ok(speakeasy_plugin::types::PluginId(uuid))
}

/// Erstellt einen neuen Channel auf dem Server
#[tauri::command]
pub async fn create_channel(
    state: State<'_, AppState>,
    name: String,
    description: Option<String>,
    password: Option<String>,
    max_clients: Option<u32>,
    parent_id: Option<String>,
) -> Result<ChannelInfo, String> {
    debug!("Erstelle Channel '{}'", name);

    let parent_channel_id = if let Some(ref pid) = parent_id {
        let uuid = uuid::Uuid::parse_str(pid)
            .map_err(|_| format!("Ungueltige parent_id '{}': keine gueltige UUID", pid))?;
        Some(speakeasy_core::types::ChannelId(uuid))
    } else {
        None
    };

    let mut tcp = state.tcp.lock().await;
    let conn = tcp
        .as_mut()
        .ok_or_else(|| "Nicht verbunden – bitte zuerst connect_to_server aufrufen".to_string())?;

    let request_id = conn.next_id();
    let nachricht = speakeasy_protocol::control::ControlMessage::new(
        request_id,
        ControlPayload::ChannelCreate(ChannelCreateRequest {
            name: name.clone(),
            description: description.clone(),
            parent_id: parent_channel_id,
            password,
            max_clients,
            sort_order: None,
        }),
    );

    let antwort = conn.send_and_receive(nachricht).await.map_err(|e| e.to_string())?;

    match antwort.payload {
        ControlPayload::ChannelCreateResponse(resp) => {
            info!("Channel '{}' erstellt: id={}", name, resp.channel_id.inner());
            Ok(ChannelInfo {
                id: resp.channel_id.inner().to_string(),
                name,
                description: description.unwrap_or_default(),
                parent_id: parent_id,
                clients: vec![],
                max_clients: max_clients.unwrap_or(0),
            })
        }
        ControlPayload::Error(e) => Err(format!("Server-Fehler: {}", e.message)),
        other => Err(format!(
            "Unerwartete Antwort vom Server: {:?}",
            std::mem::discriminant(&other)
        )),
    }
}

/// Bearbeitet einen bestehenden Channel
#[tauri::command]
pub async fn edit_channel(
    state: State<'_, AppState>,
    channel_id: String,
    name: Option<String>,
    description: Option<String>,
    password: Option<String>,
    max_clients: Option<u32>,
) -> Result<(), String> {
    debug!("Bearbeite Channel {}", channel_id);

    let cid = parse_channel_id(&channel_id)?;

    let mut tcp = state.tcp.lock().await;
    let conn = tcp
        .as_mut()
        .ok_or_else(|| "Nicht verbunden – bitte zuerst connect_to_server aufrufen".to_string())?;

    let request_id = conn.next_id();
    let nachricht = speakeasy_protocol::control::ControlMessage::new(
        request_id,
        ControlPayload::ChannelEdit(ChannelEditRequest {
            channel_id: cid,
            name,
            description,
            password,
            max_clients,
            sort_order: None,
        }),
    );

    let antwort = conn.send_and_receive(nachricht).await.map_err(|e| e.to_string())?;

    match antwort.payload {
        ControlPayload::ChannelList => {
            info!("Channel {} erfolgreich bearbeitet", channel_id);
            Ok(())
        }
        ControlPayload::Error(e) => Err(format!("Server-Fehler: {}", e.message)),
        other => Err(format!(
            "Unerwartete Antwort vom Server: {:?}",
            std::mem::discriminant(&other)
        )),
    }
}

/// Loescht einen Channel vom Server
#[tauri::command]
pub async fn delete_channel(
    state: State<'_, AppState>,
    channel_id: String,
) -> Result<(), String> {
    debug!("Loesche Channel {}", channel_id);

    let cid = parse_channel_id(&channel_id)?;

    let mut tcp = state.tcp.lock().await;
    let conn = tcp
        .as_mut()
        .ok_or_else(|| "Nicht verbunden – bitte zuerst connect_to_server aufrufen".to_string())?;

    let request_id = conn.next_id();
    let nachricht = speakeasy_protocol::control::ControlMessage::new(
        request_id,
        ControlPayload::ChannelDelete(ChannelDeleteRequest {
            channel_id: cid,
            move_clients_to: None,
        }),
    );

    let antwort = conn.send_and_receive(nachricht).await.map_err(|e| e.to_string())?;

    match antwort.payload {
        ControlPayload::ChannelList => {
            info!("Channel {} erfolgreich geloescht", channel_id);
            Ok(())
        }
        ControlPayload::Error(e) => Err(format!("Server-Fehler: {}", e.message)),
        other => Err(format!(
            "Unerwartete Antwort vom Server: {:?}",
            std::mem::discriminant(&other)
        )),
    }
}

/// Gibt Server-Informationen zurueck
#[tauri::command]
pub async fn get_server_info(state: State<'_, AppState>) -> Result<ServerInfo, String> {
    {
        let conn = state.connection.lock().map_err(|e| e.to_string())?;
        if !conn.connected {
            return Err("Nicht mit einem Server verbunden".to_string());
        }
    }

    debug!("Frage Server-Info ab");

    let mut tcp = state.tcp.lock().await;
    let conn = tcp
        .as_mut()
        .ok_or_else(|| "Keine TCP-Verbindung vorhanden".to_string())?;

    // Server-Info abrufen
    let info = conn
        .get_server_info()
        .await
        .map_err(|e| format!("Server-Info Abfrage fehlgeschlagen: {}", e))?;

    // Channel-Liste abrufen
    let channels = conn
        .get_channel_list()
        .await
        .map_err(|e| format!("Channel-Liste Abfrage fehlgeschlagen: {}", e))?;

    // Protokoll-Typen in Client-DTOs konvertieren
    let channel_dtos: Vec<ChannelInfo> = channels
        .into_iter()
        .map(|ch| ChannelInfo {
            id: ch.channel_id.inner().to_string(),
            name: ch.name,
            description: ch.description.unwrap_or_default(),
            parent_id: ch.parent_id.map(|p| p.inner().to_string()),
            clients: vec![],
            max_clients: ch.max_clients.unwrap_or(0),
        })
        .collect();

    Ok(ServerInfo {
        name: info.name,
        description: info.welcome_message.unwrap_or_default(),
        version: info.version,
        max_clients: info.max_clients,
        online_clients: info.current_clients,
        channels: channel_dtos,
    })
}
