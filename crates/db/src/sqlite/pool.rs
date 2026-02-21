//! SQLite Connection Pool mit WAL-Modus

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions};
use std::str::FromStr;
use tracing::info;

use crate::error::DbError;
use crate::repository::DatabaseConfig;

/// Wrapper um den SQLite Connection Pool
#[derive(Debug, Clone)]
pub struct SqliteDb {
    pub(crate) pool: SqlitePool,
}

impl SqliteDb {
    /// Erstellt einen neuen Pool, fuehrt Migrationen aus
    pub async fn oeffnen(config: &DatabaseConfig) -> Result<Self, DbError> {
        let opts = SqliteConnectOptions::from_str(&config.url)?
            .create_if_missing(true)
            .journal_mode(if config.sqlite_wal {
                SqliteJournalMode::Wal
            } else {
                SqliteJournalMode::Delete
            })
            .foreign_keys(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(config.max_verbindungen)
            .connect_with(opts)
            .await?;

        info!(url = %config.url, wal = config.sqlite_wal, "SQLite-Pool geoeffnet");

        let db = Self { pool };
        db.migrationen_ausfuehren().await?;

        Ok(db)
    }

    /// Fuehrt alle ausstehenden Migrationen aus
    pub async fn migrationen_ausfuehren(&self) -> Result<(), DbError> {
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await?;
        info!("Datenbank-Migrationen abgeschlossen");
        Ok(())
    }

    /// Gibt den internen Pool zurueck (fuer Tests)
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Erstellt eine In-Memory-Datenbank fuer Tests
    pub async fn in_memory() -> Result<Self, DbError> {
        let opts = SqliteConnectOptions::from_str("sqlite::memory:")?
            .foreign_keys(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            // In-Memory benoetigt mindestens 1 persistente Verbindung
            .min_connections(1)
            .connect_with(opts)
            .await?;

        let db = Self { pool };
        db.migrationen_ausfuehren().await?;
        Ok(db)
    }
}
