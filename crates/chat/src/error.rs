//! Fehlertypen fuer das Chat-Crate

use thiserror::Error;

/// Chat-Fehlertypen
#[derive(Debug, Error)]
pub enum ChatError {
    #[error("Nachricht nicht gefunden: {0}")]
    NachrichtNichtGefunden(String),

    #[error("Datei nicht gefunden: {0}")]
    DateiNichtGefunden(String),

    #[error("Keine Berechtigung: {0}")]
    KeineBerechtigung(String),

    #[error("Datei zu gross: {size} Bytes (Maximum: {max} Bytes)")]
    DateiZuGross { size: i64, max: i64 },

    #[error("Speicherkontingent erschoepft: {used} von {max} Bytes belegt")]
    KontingentErschoepft { used: i64, max: i64 },

    #[error("Ungueltige Eingabe: {0}")]
    UngueltigeEingabe(String),

    #[error("Speicher-Fehler: {0}")]
    SpeicherFehler(String),

    #[error("Datenbank-Fehler: {0}")]
    DatenbankFehler(#[from] speakeasy_db::DbError),

    #[error("IO-Fehler: {0}")]
    Io(#[from] std::io::Error),

    #[error("Unerwarteter Fehler: {0}")]
    Anyhow(#[from] anyhow::Error),
}

pub type ChatResult<T> = Result<T, ChatError>;
