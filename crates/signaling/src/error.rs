//! Fehlertypen fuer den Signaling-Service

use speakeasy_auth::AuthError;
use thiserror::Error;

/// Fehlertyp fuer den Signaling-Service
#[derive(Debug, Error)]
pub enum SignalingError {
    /// IO-Fehler (TCP, Socket)
    #[error("IO-Fehler: {0}")]
    Io(#[from] std::io::Error),

    /// Authentifizierungsfehler
    #[error("Authentifizierungsfehler: {0}")]
    Auth(#[from] AuthError),

    /// Verbindung wurde getrennt
    #[error("Verbindung getrennt")]
    VerbindungGetrennt,

    /// Protokollfehler (ungueltiges Frame, falscher Zustand)
    #[error("Protokollfehler: {0}")]
    Protokoll(String),

    /// Berechtigung verweigert
    #[error("Berechtigung verweigert: {0}")]
    ZugriffVerweigert(String),

    /// Ressource nicht gefunden
    #[error("Nicht gefunden: {0}")]
    NichtGefunden(String),

    /// Kanal ist voll
    #[error("Kanal ist voll")]
    KanalVoll,

    /// Kanal-Passwort fehlt oder falsch
    #[error("Kanal-Passwort fehlt oder falsch")]
    KanalPasswort,

    /// Benutzer ist gebannt
    #[error("Benutzer ist gebannt: {0}")]
    Gebannt(String),

    /// Server ist voll
    #[error("Server ist voll")]
    ServerVoll,

    /// Senden an Client fehlgeschlagen (Channel geschlossen)
    #[error("Senden fehlgeschlagen")]
    SendFehler,

    /// Timeout (Keepalive, Session)
    #[error("Timeout")]
    Timeout,

    /// Interner Fehler
    #[error("Interner Fehler: {0}")]
    Intern(String),
}

impl SignalingError {
    /// Erstellt einen internen Fehler
    pub fn intern(msg: impl Into<String>) -> Self {
        Self::Intern(msg.into())
    }

    /// Erstellt einen Protokollfehler
    pub fn protokoll(msg: impl Into<String>) -> Self {
        Self::Protokoll(msg.into())
    }
}

/// Result-Typ fuer den Signaling-Service
pub type SignalingResult<T> = Result<T, SignalingError>;
