//! speakeasy-audio – Client Audio Engine
//!
//! Vollstaendige Audio-Pipeline fuer Speakeasy:
//! - Mikrofon-Capture via cpal
//! - Lautsprecher-Playback via cpal
//! - Opus Encoding/Decoding
//! - DSP: Noise Gate, VAD, AGC, Noise Suppression, Echo Cancellation, De-Esser
//! - Push-to-Talk (Hold, Toggle, Voice Activation)
//! - Auto-Kalibrierung
//! - Per-User Lautstaerke-Kontrolle

pub mod calibration;
pub mod capture;
pub mod codec;
pub mod device;
pub mod dsp;
pub mod engine;
pub mod error;
pub mod pipeline;
pub mod playback;
pub mod ptt;
pub mod volume;

// Bequeme Re-Exporte der wichtigsten Typen
pub use calibration::{calibrate_from_samples, default_calibration, CalibrationResult};
pub use capture::{CaptureConfig, CaptureConsumer, CaptureProducer};
pub use codec::{OpusDecoder, OpusEncoder};
pub use device::{
    get_default_input, get_default_output, list_input_devices, list_output_devices, AudioDevice,
};
pub use dsp::AudioProcessor;
pub use engine::{AudioEngine, AudioEngineConfig, AudioStats};
pub use error::{AudioError, AudioResult};
pub use pipeline::{
    build_default_capture_pipeline, build_minimal_capture_pipeline, AudioPipeline, ProcessedFrame,
};
pub use playback::{PlaybackConfig, PlaybackProducer};
pub use ptt::{PttController, PttMode};
pub use volume::VolumeController;
