//! Voice-Client – End-to-End Audio-Pipeline
//!
//! Verbindet Mikrofon-Capture, DSP, Opus-Encoding, UDP-Transport,
//! Opus-Decoding und Playback zu einer vollstaendigen Voice-Pipeline.
//!
//! ## Sende-Pipeline (Capture -> Server)
//! ```text
//! cpal Capture Callback
//!     -> Ring-Buffer (lock-free, ringbuf)
//!     -> Processing Thread: Frames sammeln (20ms = 960 Samples bei 48kHz)
//!     -> DSP Pipeline: NoiseGate -> NoiseSuppression -> AGC
//!     -> Opus Encode: PCM f32 -> Opus bytes
//!     -> VoicePacket: Header (sequence++, timestamp, ssrc) + Opus Payload
//!     -> UDP Socket send_to(server_addr)
//! ```
//!
//! ## Empfangs-Pipeline (Server -> Playback)
//! ```text
//! UDP Socket recv_from()
//!     -> VoicePacket parse (Header + Payload)
//!     -> Opus Decode: Opus bytes -> PCM f32
//!     -> Volume Control
//!     -> Playback Ring-Buffer
//!     -> cpal Playback Callback liest aus Ring-Buffer
//! ```

use ringbuf::traits::{Consumer, Producer};
use speakeasy_audio::codec::{OpusDecoder, OpusEncoder};
use speakeasy_audio::pipeline::build_minimal_capture_pipeline;
use speakeasy_audio::volume::VolumeController;
use speakeasy_protocol::codec::{AudioPreset, OpusConfig};
use speakeasy_protocol::voice::{VoiceFlags, VoicePacket, VoicePacketHeader};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use tokio::net::UdpSocket;
use tracing::{debug, error, info, trace, warn};

/// Frame-Groesse: 20ms bei 48kHz Mono = 960 Samples
const FRAME_SIZE: usize = 960;
/// Abtastrate
const SAMPLE_RATE: u32 = 48000;
/// Maximale UDP-Paketgroesse
const UDP_BUFFER_SIZE: usize = 1400;

// ---------------------------------------------------------------------------
// VoiceClient
// ---------------------------------------------------------------------------

/// Voice-Client: Verwaltet die gesamte Audio-Pipeline
///
/// Lifecycle:
/// 1. `new()` – Erstellt den Client (noch nicht verbunden)
/// 2. `start()` – Startet UDP, Opus, Capture, Playback, Sende- und Empfangs-Tasks
/// 3. `set_muted()` / `set_deafened()` – Steuert Sende/Empfang
/// 4. `stop()` – Stoppt alles sauber
///
/// Hinweis: cpal::Stream ist !Send, daher werden die Audio-Streams
/// in einem dedizierten std::thread gehalten (audio_thread), nicht
/// im VoiceClient selbst. So bleibt VoiceClient Send+Sync fuer Tauri.
pub struct VoiceClient {
    /// SSRC vom Server zugewiesen
    ssrc: u32,
    /// Server UDP-Adresse
    server_addr: SocketAddr,
    /// Laeuft die Pipeline?
    running: Arc<AtomicBool>,
    /// Ist das Mikrofon gemutet?
    muted: Arc<AtomicBool>,
    /// Ist der Ton deaktiviert (deaf)?
    deafened: Arc<AtomicBool>,
    /// Spricht der Benutzer gerade?
    speaking: Arc<AtomicBool>,
    /// Sequenznummer fuer ausgehende Pakete
    sequence: Arc<AtomicU32>,
    /// Shutdown-Signal fuer den Empfangs-Task
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
    /// Audio-Thread: haelt cpal-Streams am Leben und fuehrt den Sende-Loop aus
    /// (std::thread weil cpal::Stream !Send ist und nicht in Tokio-Tasks leben kann)
    audio_thread: Option<std::thread::JoinHandle<()>>,
    /// Empfangs-Task (async, in Tokio)
    recv_task: Option<tokio::task::JoinHandle<()>>,
}

impl VoiceClient {
    /// Erstellt einen neuen VoiceClient (noch nicht verbunden)
    pub fn new() -> Self {
        Self {
            ssrc: 0,
            server_addr: SocketAddr::from(([0, 0, 0, 0], 0)),
            running: Arc::new(AtomicBool::new(false)),
            muted: Arc::new(AtomicBool::new(false)),
            deafened: Arc::new(AtomicBool::new(false)),
            speaking: Arc::new(AtomicBool::new(false)),
            sequence: Arc::new(AtomicU32::new(0)),
            shutdown_tx: None,
            audio_thread: None,
            recv_task: None,
        }
    }

    /// Startet die Voice-Pipeline
    ///
    /// 1. UDP-Socket oeffnen (OS waehlt Port)
    /// 2. Audio-Thread starten (haelt cpal-Streams + fuehrt Sende-Loop aus)
    /// 3. Empfangs-Task starten (async, schreibt in Playback-Ring-Buffer)
    pub async fn start(&mut self, server_addr: SocketAddr, ssrc: u32) -> Result<(), String> {
        if self.running.load(Ordering::Relaxed) {
            return Err("Voice-Pipeline laeuft bereits".to_string());
        }

        self.ssrc = ssrc;
        self.server_addr = server_addr;
        self.sequence.store(0, Ordering::Relaxed);

        info!(
            server = %server_addr,
            ssrc,
            "Starte Voice-Pipeline"
        );

        // 1. UDP-Socket binden (Port 0 = OS waehlt)
        let udp_socket = UdpSocket::bind("0.0.0.0:0")
            .await
            .map_err(|e| format!("UDP-Socket konnte nicht gebunden werden: {}", e))?;

        let local_port = udp_socket
            .local_addr()
            .map_err(|e| e.to_string())?
            .port();
        info!(port = local_port, "UDP-Socket gebunden");

        let socket = Arc::new(udp_socket);

        // Opus-Config fuer beide Loops
        let opus_config = AudioPreset::Balanced.config();

        // Shared Flags
        let running = Arc::clone(&self.running);
        let muted = Arc::clone(&self.muted);
        let speaking = Arc::clone(&self.speaking);
        let sequence = Arc::clone(&self.sequence);
        let deafened = Arc::clone(&self.deafened);

        // 2. Audio-Thread starten
        // Dieser Thread:
        //   a) Oeffnet cpal Capture + Playback Streams (diese sind !Send)
        //   b) Fuehrt den Sende-Loop aus (blockierend)
        //   c) Haelt die Streams am Leben bis running=false
        //   d) Gibt den PlaybackProducer via Channel an den Empfangs-Task
        let send_socket = Arc::clone(&socket);
        let audio_running = Arc::clone(&running);
        let audio_muted = Arc::clone(&muted);
        let audio_speaking = Arc::clone(&speaking);
        let audio_sequence = Arc::clone(&sequence);
        let audio_server_addr = self.server_addr;
        let audio_ssrc = self.ssrc;
        let audio_opus_config = opus_config.clone();

        // Channel um den PlaybackProducer vom Audio-Thread zum Empfangs-Task zu uebergeben
        let (producer_tx, producer_rx) =
            std::sync::mpsc::sync_channel::<speakeasy_audio::PlaybackProducer>(1);

        let audio_thread = std::thread::Builder::new()
            .name("voice-audio".to_string())
            .spawn(move || {
                // Audio-Streams oeffnen (cpal::Stream lebt hier im Thread)
                let streams = Self::start_audio_streams();
                let (capture_consumer, playback_producer, _capture_stream, _playback_stream) =
                    match streams {
                        Ok((cs, cc, ps, pp)) => (cc, pp, cs, ps),
                        Err(e) => {
                            error!("Audio-Streams konnten nicht geoeffnet werden: {}", e);
                            // Leeren Producer senden geht nicht, also Channel droppen
                            drop(producer_tx);
                            return;
                        }
                    };

                // PlaybackProducer an den Empfangs-Task uebergeben
                if producer_tx.send(playback_producer).is_err() {
                    error!("Empfangs-Task hat PlaybackProducer nicht abgeholt");
                    return;
                }

                // Sende-Loop blockierend ausfuehren
                // _capture_stream und _playback_stream bleiben im Scope am Leben
                Self::sende_loop(
                    capture_consumer,
                    send_socket,
                    audio_server_addr,
                    audio_ssrc,
                    audio_opus_config,
                    audio_running,
                    audio_muted,
                    audio_speaking,
                    audio_sequence,
                );

                debug!("Audio-Thread beendet, cpal-Streams werden gedroppt");
                // _capture_stream und _playback_stream werden hier gedroppt
            })
            .map_err(|e| format!("Audio-Thread konnte nicht gestartet werden: {}", e))?;

        // PlaybackProducer vom Audio-Thread empfangen
        let playback_producer = producer_rx
            .recv()
            .map_err(|_| "Audio-Streams konnten nicht initialisiert werden".to_string())?;

        // 3. Empfangs-Task starten (async)
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        let recv_running = Arc::clone(&self.running);

        let recv_task = tokio::spawn(Self::empfangs_loop(
            socket,
            playback_producer,
            opus_config,
            recv_running,
            deafened,
            shutdown_rx,
        ));

        self.running.store(true, Ordering::Relaxed);
        self.shutdown_tx = Some(shutdown_tx);
        self.audio_thread = Some(audio_thread);
        self.recv_task = Some(recv_task);

        info!("Voice-Pipeline gestartet");
        Ok(())
    }

    /// Stoppt die Voice-Pipeline sauber
    pub async fn stop(&mut self) {
        if !self.running.load(Ordering::Relaxed) {
            return;
        }

        info!("Stoppe Voice-Pipeline");

        self.running.store(false, Ordering::Relaxed);

        // Shutdown-Signal an Empfangs-Task senden
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }

        // Auf Empfangs-Task warten
        if let Some(handle) = self.recv_task.take() {
            let _ = handle.await;
        }

        // Auf Audio-Thread warten (haelt cpal-Streams, Sende-Loop)
        if let Some(handle) = self.audio_thread.take() {
            let _ = handle.join();
        }

        info!("Voice-Pipeline gestoppt");
    }

    /// Mikrofon muten/unmuten
    ///
    /// Bei Mute: Sende-Thread sendet nichts, Empfang laeuft weiter
    pub fn set_muted(&self, muted: bool) {
        self.muted.store(muted, Ordering::Relaxed);
        info!("Voice Mute: {}", muted);
    }

    /// Ton deaktivieren/aktivieren (deaf)
    ///
    /// Bei Deaf: Empfangs-Thread verwirft Pakete, Sende-Thread stoppt ebenfalls
    pub fn set_deafened(&self, deafened: bool) {
        self.deafened.store(deafened, Ordering::Relaxed);
        if deafened {
            // Deaf impliziert Mute
            self.muted.store(true, Ordering::Relaxed);
        }
        info!("Voice Deaf: {}", deafened);
    }

    /// Gibt zurueck ob der Benutzer gerade spricht
    pub fn is_speaking(&self) -> bool {
        self.speaking.load(Ordering::Relaxed)
    }

    /// Gibt zurueck ob die Pipeline laeuft
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Gibt die zugewiesene SSRC zurueck
    pub fn ssrc(&self) -> u32 {
        self.ssrc
    }

    /// Gibt den lokalen UDP-Port zurueck (fuer VoiceInit)
    pub fn local_udp_port(&self) -> u16 {
        // Wird beim Start gesetzt
        0
    }

    // -----------------------------------------------------------------------
    // Audio-Streams oeffnen
    // -----------------------------------------------------------------------

    /// Oeffnet Capture- und Playback-Streams
    fn start_audio_streams() -> Result<
        (
            speakeasy_audio::capture::CaptureStream,
            speakeasy_audio::CaptureConsumer,
            speakeasy_audio::playback::PlaybackStream,
            speakeasy_audio::PlaybackProducer,
        ),
        String,
    > {
        use cpal::traits::HostTrait;

        let host = cpal::default_host();

        // Eingabegeraet
        let input_device = host
            .default_input_device()
            .ok_or_else(|| "Kein Standard-Eingabegeraet gefunden".to_string())?;

        let capture_config = speakeasy_audio::CaptureConfig {
            sample_rate: SAMPLE_RATE,
            channels: 1,
            buffer_size: SAMPLE_RATE as usize * 2, // 2 Sekunden
        };

        let (capture_stream, capture_consumer) =
            speakeasy_audio::capture::open_capture_stream(&input_device, capture_config)
                .map_err(|e| format!("Capture-Stream konnte nicht geoeffnet werden: {}", e))?;

        // Ausgabegeraet
        let output_device = host
            .default_output_device()
            .ok_or_else(|| "Kein Standard-Ausgabegeraet gefunden".to_string())?;

        let playback_config = speakeasy_audio::PlaybackConfig {
            sample_rate: SAMPLE_RATE,
            channels: 1,
            buffer_size: SAMPLE_RATE as usize * 2,
        };

        let (playback_stream, playback_producer) =
            speakeasy_audio::playback::open_playback_stream(&output_device, playback_config)
                .map_err(|e| format!("Playback-Stream konnte nicht geoeffnet werden: {}", e))?;

        debug!("Audio-Streams geoeffnet (Capture + Playback)");

        Ok((
            capture_stream,
            capture_consumer,
            playback_stream,
            playback_producer,
        ))
    }

    // -----------------------------------------------------------------------
    // Sende-Loop (laeuft im Audio-Thread da cpal-Consumer synchron ist)
    // -----------------------------------------------------------------------

    /// Sende-Loop: Liest Frames aus dem Capture-Ring-Buffer, verarbeitet sie
    /// durch die DSP-Pipeline, enkodiert mit Opus und sendet per UDP.
    fn sende_loop(
        mut capture_consumer: speakeasy_audio::CaptureConsumer,
        socket: Arc<UdpSocket>,
        server_addr: SocketAddr,
        ssrc: u32,
        opus_config: OpusConfig,
        running: Arc<AtomicBool>,
        muted: Arc<AtomicBool>,
        speaking: Arc<AtomicBool>,
        sequence: Arc<AtomicU32>,
    ) {
        // Opus Encoder erstellen
        let mut encoder = match OpusEncoder::new(opus_config) {
            Ok(enc) => enc,
            Err(e) => {
                error!("Opus-Encoder konnte nicht erstellt werden: {}", e);
                return;
            }
        };

        // DSP-Pipeline erstellen (minimale Pipeline, kein Panic in Audio-Thread)
        let mut pipeline = build_minimal_capture_pipeline();

        // Frame-Buffer fuer das Sammeln von Samples
        let frame_size = encoder.frame_size();
        let mut frame_buffer = Vec::with_capacity(frame_size * 2);
        let mut temp_buf = vec![0.0f32; frame_size];

        // Speaking-State fuer Flags
        let mut was_speaking = false;

        debug!("Sende-Loop gestartet (frame_size={})", frame_size);

        while running.load(Ordering::Relaxed) {
            // Samples aus dem Ring-Buffer lesen
            let available = capture_consumer.pop_slice(&mut temp_buf);

            if available == 0 {
                // Kein Sample verfuegbar -> kurz schlafen (5ms = 1/4 Frame bei 20ms)
                std::thread::sleep(std::time::Duration::from_millis(5));
                continue;
            }

            // Gelesene Samples in den Frame-Buffer anhaengen
            frame_buffer.extend_from_slice(&temp_buf[..available]);

            // Frames verarbeiten sobald genug Samples vorhanden
            while frame_buffer.len() >= frame_size {
                let frame: Vec<f32> = frame_buffer.drain(..frame_size).collect();

                // Gemutet? -> Nichts senden
                if muted.load(Ordering::Relaxed) {
                    if was_speaking {
                        speaking.store(false, Ordering::Relaxed);
                        was_speaking = false;
                    }
                    continue;
                }

                // DSP-Pipeline
                let processed = pipeline.process_frame(&frame);

                // Sprach-Erkennung: RMS-Pegel pruefen
                let rms = rms_level(&processed.samples);
                let is_voice = rms > 0.005; // -46 dBFS Schwelle

                // Speaking-Flags fuer den Header
                let mut flags: u16 = 0;
                if is_voice && !was_speaking {
                    flags |= VoiceFlags::SPEAKING_START;
                    speaking.store(true, Ordering::Relaxed);
                } else if !is_voice && was_speaking {
                    flags |= VoiceFlags::SPEAKING_STOP;
                    speaking.store(false, Ordering::Relaxed);
                }
                was_speaking = is_voice;

                // Opus Encode
                let opus_bytes = match encoder.encode(&processed.samples) {
                    Ok(bytes) => bytes,
                    Err(e) => {
                        warn!("Opus-Encoding fehlgeschlagen: {}", e);
                        continue;
                    }
                };

                // Sequenz und Zeitstempel
                let seq = sequence.fetch_add(1, Ordering::Relaxed);
                let timestamp = seq * frame_size as u32;

                // VoicePacket erstellen
                let paket = if is_voice {
                    VoicePacket {
                        header: VoicePacketHeader::new(
                            speakeasy_protocol::voice::PacketType::Audio,
                            flags,
                            seq,
                            timestamp,
                            ssrc,
                        ),
                        payload: opus_bytes,
                    }
                } else {
                    // Silence-Paket (DTX)
                    VoicePacket::neu_silence(seq, timestamp, ssrc)
                };

                // UDP senden (blockierend im spawn_blocking-Kontext)
                let encoded = paket.encode();

                // Wir nutzen try_send via std::net::UdpSocket, da wir in einem
                // blockierenden Thread laufen. socket.try_send_to blockiert nicht.
                if let Err(e) = socket.try_send_to(&encoded, server_addr) {
                    trace!("UDP-Sendefehler: {}", e);
                }
            }
        }

        debug!("Sende-Loop beendet");
    }

    // -----------------------------------------------------------------------
    // Empfangs-Loop (async, laeuft in Tokio-Task)
    // -----------------------------------------------------------------------

    /// Empfangs-Loop: Empfaengt UDP-Pakete, dekodiert Opus und schreibt in den
    /// Playback-Ring-Buffer.
    async fn empfangs_loop(
        socket: Arc<UdpSocket>,
        mut playback_producer: speakeasy_audio::PlaybackProducer,
        opus_config: OpusConfig,
        running: Arc<AtomicBool>,
        deafened: Arc<AtomicBool>,
        mut shutdown_rx: tokio::sync::oneshot::Receiver<()>,
    ) {
        // Opus Decoder erstellen
        let mut decoder =
            match OpusDecoder::from_config(&opus_config) {
                Ok(dec) => dec,
                Err(e) => {
                    error!("Opus-Decoder konnte nicht erstellt werden: {}", e);
                    return;
                }
            };

        // Volume Controller (wird spaeter fuer per-User Volume genutzt)
        let _volume = VolumeController::new();

        let mut buf = [0u8; UDP_BUFFER_SIZE];

        debug!("Empfangs-Loop gestartet");

        loop {
            tokio::select! {
                result = socket.recv_from(&mut buf) => {
                    match result {
                        Ok((len, _absender)) => {
                            // Deaf? -> Paket verwerfen
                            if deafened.load(Ordering::Relaxed) {
                                continue;
                            }

                            // Pipeline gestoppt?
                            if !running.load(Ordering::Relaxed) {
                                break;
                            }

                            // VoicePacket dekodieren
                            let paket = match VoicePacket::decode(&buf[..len]) {
                                Ok(p) => p,
                                Err(e) => {
                                    trace!("Ungueltiges Voice-Paket: {}", e);
                                    continue;
                                }
                            };

                            // Silence-Pakete ueberspringen (kein Decode noetig)
                            if paket.header.packet_type == speakeasy_protocol::voice::PacketType::Silence {
                                continue;
                            }

                            // Opus Decode
                            let pcm = match decoder.decode(&paket.payload) {
                                Ok(samples) => samples,
                                Err(e) => {
                                    trace!("Opus-Decoding fehlgeschlagen: {}", e);
                                    // PLC (Packet Loss Concealment)
                                    match decoder.decode_plc() {
                                        Ok(plc) => plc,
                                        Err(_) => continue,
                                    }
                                }
                            };

                            // In Playback-Ring-Buffer schreiben
                            let written = playback_producer.push_slice(&pcm);
                            if written < pcm.len() {
                                trace!(
                                    "Playback Ring-Buffer voll: {} von {} Samples geschrieben",
                                    written,
                                    pcm.len()
                                );
                            }
                        }
                        Err(e) => {
                            if running.load(Ordering::Relaxed) {
                                warn!("UDP-Empfangsfehler: {}", e);
                            }
                        }
                    }
                }

                // Shutdown-Signal
                _ = &mut shutdown_rx => {
                    debug!("Empfangs-Loop: Shutdown-Signal empfangen");
                    break;
                }
            }
        }

        debug!("Empfangs-Loop beendet");
    }
}

impl Drop for VoiceClient {
    fn drop(&mut self) {
        // Sicherstellen dass alles gestoppt wird
        self.running.store(false, Ordering::Relaxed);
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        // Audio-Thread joinen (blockiert kurz, aber Drop ist synchron)
        if let Some(handle) = self.audio_thread.take() {
            let _ = handle.join();
        }
        debug!("VoiceClient gedroppt");
    }
}

// ---------------------------------------------------------------------------
// Hilfsfunktionen
// ---------------------------------------------------------------------------

/// Berechnet den RMS-Pegel eines Audio-Frames
fn rms_level(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = samples.iter().map(|s| s * s).sum();
    (sum_sq / samples.len() as f32).sqrt()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rms_stille() {
        let silence = vec![0.0f32; 960];
        assert!(rms_level(&silence) < f32::EPSILON);
    }

    #[test]
    fn rms_signal() {
        let signal = vec![0.5f32; 960];
        let rms = rms_level(&signal);
        assert!((rms - 0.5).abs() < 0.01);
    }

    #[test]
    fn rms_leer() {
        assert!(rms_level(&[]) < f32::EPSILON);
    }

    #[test]
    fn voice_client_erstellen() {
        let client = VoiceClient::new();
        assert!(!client.is_running());
        assert!(!client.is_speaking());
        assert_eq!(client.ssrc(), 0);
    }

    #[test]
    fn voice_client_mute_flags() {
        let client = VoiceClient::new();
        assert!(!client.muted.load(Ordering::Relaxed));
        client.set_muted(true);
        assert!(client.muted.load(Ordering::Relaxed));
        client.set_muted(false);
        assert!(!client.muted.load(Ordering::Relaxed));
    }

    #[test]
    fn voice_client_deafen_impliziert_mute() {
        let client = VoiceClient::new();
        client.set_deafened(true);
        assert!(client.deafened.load(Ordering::Relaxed));
        assert!(client.muted.load(Ordering::Relaxed));
    }
}
