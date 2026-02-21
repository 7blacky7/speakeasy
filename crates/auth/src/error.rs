//! Fehlertypen fuer den Auth-Service

use thiserror::Error;

/// Alle moeglichen Fehler im Auth-Service
#[derive(Debug, Error)]
pub enum AuthError {
    // --- Passwort ---
    #[error("Passwort-Hashing fehlgeschlagen: {0}")]
    PasswortHashing(String),

    // --- Authentifizierung ---
    #[error("Benutzername oder Passwort falsch")]
    UngueltigeAnmeldedaten,

    #[error("Benutzer gesperrt")]
    BenutzerGesperrt,

    #[error("Benutzer gebannt: {0}")]
    BenutzerGebannt(String),

    #[error("IP gebannt: {0}")]
    IpGebannt(String),

    // --- Session ---
    #[error("Session nicht gefunden oder abgelaufen")]
    SessionUngueltig,

    #[error("Session abgelaufen")]
    SessionAbgelaufen,

    // --- API-Token ---
    #[error("API-Token ungueltig")]
    TokenUngueltig,

    #[error("API-Token abgelaufen")]
    TokenAbgelaufen,

    #[error("API-Token hat nicht den benoetigten Scope: {0}")]
    TokenScopeFehlend(String),

    // --- Berechtigungen ---
    #[error("Zugriff verweigert: Berechtigung '{0}' fehlt")]
    ZugriffVerweigert(String),

    // --- Benutzerverwaltung ---
    #[error("Benutzername bereits vergeben: {0}")]
    BenutzernameVergeben(String),

    #[error("Benutzer nicht gefunden: {0}")]
    BenutzerNichtGefunden(String),

    // --- Einladungen ---
    #[error("Einladungscode ungueltig oder abgelaufen")]
    EinladungUngueltig,

    #[error("Einladung erschoepft")]
    EinladungErschoepft,

    // --- Datenbank ---
    #[error("Datenbankfehler: {0}")]
    Datenbank(#[from] speakeasy_db::DbError),

    // --- Intern ---
    #[error("Interner Fehler: {0}")]
    Intern(String),
}

impl AuthError {
    pub fn intern(msg: impl Into<String>) -> Self {
        Self::Intern(msg.into())
    }
}

/// Result-Alias fuer den Auth-Service
pub type AuthResult<T> = Result<T, AuthError>;
