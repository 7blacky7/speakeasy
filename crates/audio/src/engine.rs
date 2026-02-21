//! AudioEngine – Zentrale Steuerung der Audio-Pipeline
//!
//! Koordiniert Capture, Playback, DSP-Pipeline, PTT und Codec.
//! Kommuniziert ueber crossbeam-channel mit dem Audio-Thread.

use crossbeam_channel::{bounded, Receiver, Sender};
use parking_lot::RwLock;
use std::sync::Arc;
use tracing::{debug, error, info};

use crate::capture::CaptureConfig;
use crate::error::{AudioError, AudioResult};
use crate::pipeline::{build_default_capture_pipeline, build_minimal_capture_pipeline};
use crate::playback::PlaybackConfig;
use crate::ptt::{PttController, PttMode};

/// Statistiken der Audio-Engine
#[derive(Debug, Clone, Default)]
pub struct AudioStats {
    /// Anzahl verarbeiteter Frames seit Start
    pub frames_processed: u64,
    /// Anzahl verworfener Frames (Buffer voll)
    pub frames_dropped: u64,
    /// Aktueller Eingangspegel (RMS, normalisiert)
    pub input_level: f32,
    /// Aktueller Ausgangspegel (RMS, normalisiert)
    pub output_level: f32,
    /// Ob aktuell gesendet wird
    pub is_transmitting: bool,
}

/// Konfiguration der Audio-Engine
#[derive(Debug, Clone)]
pub struct AudioEngineConfig {
    /// Name des Eingabegeraets (None = Standard)
    pub input_device: Option<String>,
    /// Name des Ausgabegeraets (None = Standard)
    pub output_device: Option<String>,
    /// Capture-Konfiguration
    pub capture: CaptureConfig,
    /// Playback-Konfiguration
    pub playback: PlaybackConfig,
    /// PTT-Modus
    pub ptt_mode: PttMode,
    /// Minimale Pipeline (ohne Echo-Cancellation und De-Esser)
    pub minimal_pipeline: bool,
}

impl Default for AudioEngineConfig {
    fn default() -> Self {
        Self {
            input_device: None,
            output_device: None,
            capture: CaptureConfig::default(),
            playback: PlaybackConfig::default(),
            ptt_mode: PttMode::VoiceActivation,
            minimal_pipeline: false,
        }
    }
}

/// Kommandos an den Audio-Thread
#[derive(Debug)]
pub enum AudioCommand {
    StartCapture,
    StopCapture,
    StartPlayback,
    StopPlayback,
    SetInputDevice(String),
    SetOutputDevice(String),
    PttKeyDown,
    PttKeyUp,
    PttToggle,
    SetMuted(bool),
    Shutdown,
}

/// Interne Engine-State (thread-safe)
struct EngineState {
    config: AudioEngineConfig,
    stats: AudioStats,
    capture_active: bool,
    playback_active: bool,
}

/// Audio-Engine
///
/// Die Engine selbst ist kein async-Typ – Audio-Callbacks laufen
/// synchron im cpal-Thread. Steuerkommandos werden ueber
/// crossbeam-channel gesendet.
pub struct AudioEngine {
    cmd_tx: Sender<AudioCommand>,
    state: Arc<RwLock<EngineState>>,
    config: AudioEngineConfig,
}

impl AudioEngine {
    /// Erstellt eine neue Audio-Engine (initialisiert aber startet noch nicht)
    pub fn new(config: AudioEngineConfig) -> AudioResult<Self> {
        let (cmd_tx, cmd_rx) = bounded::<AudioCommand>(64);

        let state = Arc::new(RwLock::new(EngineState {
            config: config.clone(),
            stats: AudioStats::default(),
            capture_active: false,
            playback_active: false,
        }));

        let state_clone = Arc::clone(&state);
        let config_clone = config.clone();

        // Hintergrund-Thread fuer Kommando-Verarbeitung
        std::thread::Builder::new()
            .name("speakeasy-audio-engine".to_string())
            .spawn(move || {
                audio_engine_thread(cmd_rx, state_clone, config_clone);
            })
            .map_err(|e| AudioError::StreamFehler(e.to_string()))?;

        info!("AudioEngine initialisiert");

        Ok(Self {
            cmd_tx,
            state,
            config,
        })
    }

    /// Startet die Mikrofon-Aufnahme
    pub fn start_capture(&self) -> AudioResult<()> {
        self.send_cmd(AudioCommand::StartCapture)
    }

    /// Stoppt die Mikrofon-Aufnahme
    pub fn stop_capture(&self) -> AudioResult<()> {
        self.send_cmd(AudioCommand::StopCapture)
    }

    /// Startet den Lautsprecher-Output
    pub fn start_playback(&self) -> AudioResult<()> {
        self.send_cmd(AudioCommand::StartPlayback)
    }

    /// Stoppt den Lautsprecher-Output
    pub fn stop_playback(&self) -> AudioResult<()> {
        self.send_cmd(AudioCommand::StopPlayback)
    }

    /// Wechselt das Eingabegeraet
    pub fn set_input_device(&self, device_id: impl Into<String>) -> AudioResult<()> {
        self.send_cmd(AudioCommand::SetInputDevice(device_id.into()))
    }

    /// Wechselt das Ausgabegeraet
    pub fn set_output_device(&self, device_id: impl Into<String>) -> AudioResult<()> {
        self.send_cmd(AudioCommand::SetOutputDevice(device_id.into()))
    }

    /// PTT-Taste gedrueckt
    pub fn ptt_key_down(&self) -> AudioResult<()> {
        self.send_cmd(AudioCommand::PttKeyDown)
    }

    /// PTT-Taste losgelassen
    pub fn ptt_key_up(&self) -> AudioResult<()> {
        self.send_cmd(AudioCommand::PttKeyUp)
    }

    /// PTT umschalten (Toggle-Modus)
    pub fn ptt_toggle(&self) -> AudioResult<()> {
        self.send_cmd(AudioCommand::PttToggle)
    }

    /// Mikrofon global muten/unmuten
    pub fn set_muted(&self, muted: bool) -> AudioResult<()> {
        self.send_cmd(AudioCommand::SetMuted(muted))
    }

    /// Gibt aktuelle Statistiken zurueck
    pub fn get_audio_stats(&self) -> AudioStats {
        self.state.read().stats.clone()
    }

    /// Gibt zurueck ob Capture aktiv ist
    pub fn is_capture_active(&self) -> bool {
        self.state.read().capture_active
    }

    /// Gibt zurueck ob Playback aktiv ist
    pub fn is_playback_active(&self) -> bool {
        self.state.read().playback_active
    }

    /// Gibt die Konfiguration zurueck
    pub fn config(&self) -> &AudioEngineConfig {
        &self.config
    }

    fn send_cmd(&self, cmd: AudioCommand) -> AudioResult<()> {
        self.cmd_tx
            .send(cmd)
            .map_err(|e| AudioError::StreamFehler(e.to_string()))
    }
}

impl Drop for AudioEngine {
    fn drop(&mut self) {
        let _ = self.cmd_tx.send(AudioCommand::Shutdown);
        debug!("AudioEngine gestoppt");
    }
}

/// Hintergrund-Thread der Audio-Engine
fn audio_engine_thread(
    cmd_rx: Receiver<AudioCommand>,
    state: Arc<RwLock<EngineState>>,
    config: AudioEngineConfig,
) {
    let mut ptt = PttController::new(config.ptt_mode);
    let mut _pipeline = if config.minimal_pipeline {
        build_minimal_capture_pipeline()
    } else {
        build_default_capture_pipeline()
    };

    debug!("Audio-Engine Thread gestartet");

    loop {
        match cmd_rx.recv() {
            Ok(cmd) => {
                match cmd {
                    AudioCommand::StartCapture => {
                        state.write().capture_active = true;
                        info!("Capture gestartet");
                    }
                    AudioCommand::StopCapture => {
                        state.write().capture_active = false;
                        info!("Capture gestoppt");
                    }
                    AudioCommand::StartPlayback => {
                        state.write().playback_active = true;
                        info!("Playback gestartet");
                    }
                    AudioCommand::StopPlayback => {
                        state.write().playback_active = false;
                        info!("Playback gestoppt");
                    }
                    AudioCommand::SetInputDevice(id) => {
                        state.write().config.input_device = Some(id.clone());
                        info!("Eingabegeraet gewechselt: {}", id);
                    }
                    AudioCommand::SetOutputDevice(id) => {
                        state.write().config.output_device = Some(id.clone());
                        info!("Ausgabegeraet gewechselt: {}", id);
                    }
                    AudioCommand::PttKeyDown => {
                        ptt.key_down();
                        state.write().stats.is_transmitting = ptt.is_transmitting();
                    }
                    AudioCommand::PttKeyUp => {
                        ptt.key_up();
                        state.write().stats.is_transmitting = ptt.is_transmitting();
                    }
                    AudioCommand::PttToggle => {
                        ptt.toggle();
                        state.write().stats.is_transmitting = ptt.is_transmitting();
                    }
                    AudioCommand::SetMuted(muted) => {
                        ptt.set_muted(muted);
                        state.write().stats.is_transmitting = ptt.is_transmitting();
                    }
                    AudioCommand::Shutdown => {
                        info!("Audio-Engine Thread beendet");
                        break;
                    }
                }
            }
            Err(e) => {
                error!("Audio-Engine Kanal-Fehler: {}", e);
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_erstellen() {
        let config = AudioEngineConfig::default();
        let engine = AudioEngine::new(config);
        assert!(engine.is_ok(), "Engine sollte erstellbar sein");
    }

    #[test]
    fn engine_stats_default() {
        let engine = AudioEngine::new(AudioEngineConfig::default()).unwrap();
        let stats = engine.get_audio_stats();
        assert_eq!(stats.frames_processed, 0);
        assert!(!stats.is_transmitting);
    }

    #[test]
    fn engine_start_stop_capture_kommando() {
        let engine = AudioEngine::new(AudioEngineConfig::default()).unwrap();
        assert!(engine.start_capture().is_ok());
        // Kurz warten damit der Thread reagiert
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(engine.is_capture_active());
        assert!(engine.stop_capture().is_ok());
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(!engine.is_capture_active());
    }

    #[test]
    fn engine_start_stop_playback_kommando() {
        let engine = AudioEngine::new(AudioEngineConfig::default()).unwrap();
        assert!(engine.start_playback().is_ok());
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(engine.is_playback_active());
        assert!(engine.stop_playback().is_ok());
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(!engine.is_playback_active());
    }

    #[test]
    fn engine_ptt_hold_modus() {
        let config = AudioEngineConfig {
            ptt_mode: PttMode::Hold,
            ..Default::default()
        };
        let engine = AudioEngine::new(config).unwrap();
        engine.ptt_key_down().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(engine.get_audio_stats().is_transmitting);
        engine.ptt_key_up().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(!engine.get_audio_stats().is_transmitting);
    }

    #[test]
    fn engine_mute() {
        let config = AudioEngineConfig {
            ptt_mode: PttMode::Hold,
            ..Default::default()
        };
        let engine = AudioEngine::new(config).unwrap();
        engine.ptt_key_down().unwrap();
        engine.set_muted(true).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(!engine.get_audio_stats().is_transmitting, "Mute sollte Sendung verhindern");
    }

    #[test]
    fn engine_geraet_wechseln() {
        let engine = AudioEngine::new(AudioEngineConfig::default()).unwrap();
        assert!(engine.set_input_device("test-device").is_ok());
        assert!(engine.set_output_device("test-device").is_ok());
    }

    #[test]
    fn engine_config_abfrage() {
        let config = AudioEngineConfig {
            ptt_mode: PttMode::Toggle,
            ..Default::default()
        };
        let engine = AudioEngine::new(config).unwrap();
        assert_eq!(engine.config().ptt_mode, PttMode::Toggle);
    }

    #[test]
    fn engine_minimale_pipeline() {
        let config = AudioEngineConfig {
            minimal_pipeline: true,
            ..Default::default()
        };
        let engine = AudioEngine::new(config);
        assert!(engine.is_ok());
    }

    #[test]
    fn audio_stats_default() {
        let stats = AudioStats::default();
        assert_eq!(stats.frames_processed, 0);
        assert_eq!(stats.frames_dropped, 0);
        assert!((stats.input_level).abs() < f32::EPSILON);
    }
}
