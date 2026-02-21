//! Fehlertypen fuer die Audio-Engine

use thiserror::Error;

/// Alle moeglichen Fehler der Audio-Engine
#[derive(Debug, Error)]
pub enum AudioError {
    #[error("Audio-Geraet nicht gefunden: {0}")]
    GeraetNichtGefunden(String),

    #[error("Kein Standard-Eingabegeraet verfuegbar")]
    KeinStandardEingabegeraet,

    #[error("Kein Standard-Ausgabegeraet verfuegbar")]
    KeinStandardAusgabegeraet,

    #[error("Stream-Fehler: {0}")]
    StreamFehler(String),

    #[error("Codec-Fehler: {0}")]
    CodecFehler(String),

    #[error("Konfigurationsfehler: {0}")]
    Konfiguration(String),

    #[error("Pipeline nicht initialisiert")]
    PipelineNichtInitialisiert,

    #[error("Kalibrierungs-Timeout")]
    KalibrierungsTimeout,

    #[error("Ring-Buffer voll")]
    RingBufferVoll,

    #[error("Ring-Buffer leer")]
    RingBufferLeer,

    #[error("IO-Fehler: {0}")]
    Io(#[from] std::io::Error),

    #[error("Unerwarteter Fehler: {0}")]
    Anyhow(#[from] anyhow::Error),
}

pub type AudioResult<T> = Result<T, AudioError>;
