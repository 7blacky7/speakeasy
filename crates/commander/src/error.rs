//! Fehlertypen fuer den Speakeasy Commander

use thiserror::Error;

/// Alle moeglichen Fehler im Commander-Crate
#[derive(Debug, Error)]
pub enum CommanderError {
    #[error("Authentifizierung fehlgeschlagen: {0}")]
    Authentifizierung(String),

    #[error("Nicht autorisiert: {0}")]
    NichtAutorisiert(String),

    #[error("Rate Limit ueberschritten: bitte warte {retry_after_secs} Sekunden")]
    RateLimitUeberschritten { retry_after_secs: u64 },

    #[error("Ressource nicht gefunden: {0}")]
    NichtGefunden(String),

    #[error("Ungueltige Eingabe: {0}")]
    UngueltigeEingabe(String),

    #[error("Datenbankfehler: {0}")]
    Datenbank(#[from] speakeasy_db::DbError),

    #[error("Auth-Fehler: {0}")]
    Auth(#[from] speakeasy_auth::AuthError),

    #[error("Interner Fehler: {0}")]
    Intern(#[from] anyhow::Error),

    #[error("IO-Fehler: {0}")]
    Io(#[from] std::io::Error),

    #[error("TLS-Fehler: {0}")]
    Tls(String),

    #[error("Protokollfehler: {0}")]
    Protokoll(String),
}

pub type CommanderResult<T> = Result<T, CommanderError>;

impl CommanderError {
    /// Fehler-Code fuer TCP-Protokoll-Antworten
    pub fn fehler_code(&self) -> u32 {
        match self {
            Self::Authentifizierung(_) => 1001,
            Self::NichtAutorisiert(_) => 1002,
            Self::RateLimitUeberschritten { .. } => 1003,
            Self::NichtGefunden(_) => 1004,
            Self::UngueltigeEingabe(_) => 1005,
            Self::Datenbank(_) => 2001,
            Self::Auth(_) => 2002,
            Self::Intern(_) => 5000,
            Self::Io(_) => 5001,
            Self::Tls(_) => 5002,
            Self::Protokoll(_) => 5003,
        }
    }
}

/// HTTP-Statuscode fuer REST-Fehler
impl CommanderError {
    pub fn http_status(&self) -> u16 {
        match self {
            Self::Authentifizierung(_) => 401,
            Self::NichtAutorisiert(_) => 403,
            Self::RateLimitUeberschritten { .. } => 429,
            Self::NichtGefunden(_) => 404,
            Self::UngueltigeEingabe(_) => 400,
            Self::Datenbank(_) | Self::Auth(_) => 500,
            Self::Intern(_) | Self::Io(_) | Self::Tls(_) | Self::Protokoll(_) => 500,
        }
    }
}
