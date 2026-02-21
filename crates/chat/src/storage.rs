//! Storage-Backend fuer Datei-Uploads
//!
//! Das `StorageBackend`-Trait abstrahiert den konkreten Speicher (Disk, S3, etc.).

use std::path::PathBuf;

use crate::error::ChatResult;

/// Abstraktes Speicher-Backend fuer Dateien
#[allow(async_fn_in_trait)]
pub trait StorageBackend: Send + Sync {
    /// Datei unter dem angegebenen Pfad speichern
    async fn store(&self, path: &str, data: &[u8]) -> ChatResult<()>;

    /// Datei laden
    async fn retrieve(&self, path: &str) -> ChatResult<Vec<u8>>;

    /// Datei loeschen
    async fn delete(&self, path: &str) -> ChatResult<()>;
}

/// Disk-basiertes Storage-Backend
///
/// Speichert Dateien unter `base_dir/<path>`.
#[derive(Debug, Clone)]
pub struct DiskStorage {
    base_dir: PathBuf,
}

impl DiskStorage {
    /// Neues DiskStorage mit dem angegebenen Basisverzeichnis erstellen
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
        }
    }

    /// Vollstaendigen Dateipfad aus relativem Pfad berechnen
    fn full_path(&self, path: &str) -> PathBuf {
        self.base_dir.join(path)
    }
}

impl StorageBackend for DiskStorage {
    async fn store(&self, path: &str, data: &[u8]) -> ChatResult<()> {
        let full = self.full_path(path);

        // Elternverzeichnis anlegen falls noetig
        if let Some(parent) = full.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        tokio::fs::write(&full, data).await?;
        tracing::debug!(path = %full.display(), bytes = data.len(), "Datei gespeichert");
        Ok(())
    }

    async fn retrieve(&self, path: &str) -> ChatResult<Vec<u8>> {
        let full = self.full_path(path);
        let data = tokio::fs::read(&full).await?;
        tracing::debug!(path = %full.display(), bytes = data.len(), "Datei gelesen");
        Ok(data)
    }

    async fn delete(&self, path: &str) -> ChatResult<()> {
        let full = self.full_path(path);
        match tokio::fs::remove_file(&full).await {
            Ok(()) => {
                tracing::debug!(path = %full.display(), "Datei geloescht");
                Ok(())
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // Bereits geloescht – kein Fehler
                Ok(())
            }
            Err(e) => Err(e.into()),
        }
    }
}
