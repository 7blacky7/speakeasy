//! Fehlertypen fuer Speakeasy
//!
//! Zentraler Fehler-Enum der alle moeglichen Fehlerzustaende abdeckt.
//! Untermodule koennen eigene Fehler definieren und via `#[from]` konvertieren.

use thiserror::Error;

/// Globaler Result-Alias fuer Speakeasy
pub type Result<T> = std::result::Result<T, SpeakeasyError>;

/// Alle moeglichen Fehler im Speakeasy-System
#[derive(Debug, Error)]
pub enum SpeakeasyError {
    // --- Verbindung & Netzwerk ---
    #[error("Verbindung fehlgeschlagen: {0}")]
    Verbindung(String),

    #[error("Verbindung getrennt: {0}")]
    Getrennt(String),

    #[error("Zeitlimit ueberschritten: {0}")]
    Zeitlimit(String),

    // --- Authentifizierung & Autorisierung ---
    #[error("Authentifizierung fehlgeschlagen: {0}")]
    Authentifizierung(String),

    #[error("Zugriff verweigert: {0}")]
    ZugriffVerweigert(String),

    #[error("Session abgelaufen")]
    SessionAbgelaufen,

    // --- Protokoll ---
    #[error("Ungueltige Nachricht: {0}")]
    UngueltigeNachricht(String),

    #[error("Protokollversion nicht unterstuetzt: erwartet={erwartet}, erhalten={erhalten}")]
    ProtokollVersion { erwartet: u16, erhalten: u16 },

    // --- Ressourcen ---
    #[error("Kanal nicht gefunden: {0}")]
    KanalNichtGefunden(String),

    #[error("Benutzer nicht gefunden: {0}")]
    BenutzerNichtGefunden(String),

    #[error("Server voll: maximale Clientanzahl erreicht")]
    ServerVoll,

    // --- Konfiguration ---
    #[error("Konfigurationsfehler: {0}")]
    Konfiguration(String),

    // --- Datenbank ---
    #[error("Datenbankfehler: {0}")]
    Datenbank(String),

    // --- Audio ---
    #[error("Audiofehler: {0}")]
    Audio(String),

    // --- Plugin ---
    #[error("Plugin-Fehler ({name}): {grund}")]
    Plugin { name: String, grund: String },

    // --- Intern ---
    #[error("Interner Fehler: {0}")]
    Intern(String),

    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
}

impl SpeakeasyError {
    /// Erstellt einen internen Fehler aus einer beliebigen Nachricht
    pub fn intern(msg: impl Into<String>) -> Self {
        Self::Intern(msg.into())
    }

    /// Gibt true zurueck wenn der Fehler wiederholbar sein koennte
    pub fn ist_wiederholbar(&self) -> bool {
        matches!(
            self,
            Self::Zeitlimit(_) | Self::Verbindung(_) | Self::Getrennt(_)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fehler_anzeige() {
        let e = SpeakeasyError::Authentifizierung("Falsches Passwort".into());
        assert_eq!(
            e.to_string(),
            "Authentifizierung fehlgeschlagen: Falsches Passwort"
        );
    }

    #[test]
    fn wiederholbar_erkennung() {
        assert!(SpeakeasyError::Zeitlimit("test".into()).ist_wiederholbar());
        assert!(!SpeakeasyError::ZugriffVerweigert("test".into()).ist_wiederholbar());
    }

    #[test]
    fn protokoll_version_fehler() {
        let e = SpeakeasyError::ProtokollVersion {
            erwartet: 1,
            erhalten: 2,
        };
        assert!(e.to_string().contains("erwartet=1"));
        assert!(e.to_string().contains("erhalten=2"));
    }
}
