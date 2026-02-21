//! Repository-Trait-Definitionen (Platzhalter)
//!
//! Das Repository-Pattern entkoppelt die Geschaeftslogik von der konkreten
//! Datenbank-Implementierung. Dieser Platzhalter wird im naechsten Task
//! mit SQLite- und PostgreSQL-Implementierungen befuellt.

use speakeasy_core::{
    error::Result,
    types::{ChannelId, UserId},
};

/// Unterstuetzte Datenbank-Backends
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DatabaseBackend {
    /// SQLite – Standard fuer Single-Instance-Betrieb
    Sqlite,
    /// PostgreSQL – fuer Multi-Instance-Betrieb
    Postgres,
}

impl std::fmt::Display for DatabaseBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sqlite => write!(f, "SQLite"),
            Self::Postgres => write!(f, "PostgreSQL"),
        }
    }
}

/// Konfiguration fuer die Datenbankverbindung
#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    /// Ausgewaehltes Backend
    pub backend: DatabaseBackend,
    /// Verbindungs-URL (z.B. "sqlite://speakeasy.db" oder "postgres://...")
    pub url: String,
    /// Maximale Anzahl gleichzeitiger Verbindungen im Pool
    pub max_verbindungen: u32,
    /// Ob WAL-Modus bei SQLite aktiviert werden soll
    pub sqlite_wal: bool,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            backend: DatabaseBackend::Sqlite,
            url: "sqlite://speakeasy.db".into(),
            max_verbindungen: 5,
            sqlite_wal: true,
        }
    }
}

/// Repository fuer Benutzer-Datenzugriffe
///
/// Platzhalter-Trait – wird in einem spaeteren Task implementiert.
#[allow(async_fn_in_trait)]
pub trait BenutzerRepository: Send + Sync {
    /// Einen Benutzer anhand seiner ID laden
    async fn laden(&self, id: UserId) -> Result<Option<BenutzerRecord>>;

    /// Einen Benutzer anhand seines Namens laden
    async fn laden_nach_name(&self, name: &str) -> Result<Option<BenutzerRecord>>;

    /// Einen neuen Benutzer anlegen
    async fn erstellen(&self, name: &str, passwort_hash: &str) -> Result<BenutzerRecord>;

    /// Einen Benutzer loeschen
    async fn loeschen(&self, id: UserId) -> Result<bool>;
}

/// Repository fuer Kanal-Datenzugriffe
///
/// Platzhalter-Trait – wird in einem spaeteren Task implementiert.
#[allow(async_fn_in_trait)]
pub trait KanalRepository: Send + Sync {
    /// Alle Kanaele laden
    async fn alle(&self) -> Result<Vec<KanalRecord>>;

    /// Einen Kanal anhand seiner ID laden
    async fn laden(&self, id: ChannelId) -> Result<Option<KanalRecord>>;

    /// Einen neuen Kanal anlegen
    async fn erstellen(&self, name: &str, beschreibung: Option<&str>) -> Result<KanalRecord>;

    /// Einen Kanal loeschen
    async fn loeschen(&self, id: ChannelId) -> Result<bool>;
}

/// Datensatz fuer einen Benutzer (Platzhalter)
#[derive(Debug, Clone)]
pub struct BenutzerRecord {
    pub id: UserId,
    pub name: String,
    pub passwort_hash: String,
    pub erstellt_am: chrono::DateTime<chrono::Utc>,
}

/// Datensatz fuer einen Kanal (Platzhalter)
#[derive(Debug, Clone)]
pub struct KanalRecord {
    pub id: ChannelId,
    pub name: String,
    pub beschreibung: Option<String>,
    pub erstellt_am: chrono::DateTime<chrono::Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn database_config_standard() {
        let cfg = DatabaseConfig::default();
        assert_eq!(cfg.backend, DatabaseBackend::Sqlite);
        assert!(cfg.sqlite_wal);
        assert_eq!(cfg.max_verbindungen, 5);
    }

    #[test]
    fn backend_anzeige() {
        assert_eq!(DatabaseBackend::Sqlite.to_string(), "SQLite");
        assert_eq!(DatabaseBackend::Postgres.to_string(), "PostgreSQL");
    }
}
