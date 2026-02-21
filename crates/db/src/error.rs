//! Fehlertypen fuer das Datenbank-Crate

use thiserror::Error;

/// Datenbank-Fehlertypen
#[derive(Debug, Error)]
pub enum DbError {
    #[error("Datensatz nicht gefunden: {0}")]
    NichtGefunden(String),

    #[error("Eindeutigkeitsverletzung: {0}")]
    Eindeutigkeit(String),

    #[error("Ungueltige Daten: {0}")]
    UngueltigeDaten(String),

    #[error("Einladung ungueltig oder abgelaufen")]
    EinladungUngueltig,

    #[error("Einladung erschoepft (max_uses erreicht)")]
    EinladungErschoepft,

    #[error("SQLx-Fehler: {0}")]
    Sqlx(#[from] sqlx::Error),

    #[error("Migration-Fehler: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),

    #[error("JSON-Fehler: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Interner DB-Fehler: {0}")]
    Intern(String),
}

impl DbError {
    pub fn nicht_gefunden(msg: impl Into<String>) -> Self {
        Self::NichtGefunden(msg.into())
    }

    pub fn intern(msg: impl Into<String>) -> Self {
        Self::Intern(msg.into())
    }

    /// Gibt true zurueck wenn es sich um einen Eindeutigkeitsfehler handelt
    pub fn ist_eindeutigkeit(&self) -> bool {
        matches!(self, Self::Eindeutigkeit(_))
            || matches!(self, Self::Sqlx(e) if {
                let msg = e.to_string();
                msg.contains("UNIQUE") || msg.contains("unique")
            })
    }
}
