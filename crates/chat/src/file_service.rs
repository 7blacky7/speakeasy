//! FileService – Datei-Upload, Download und Loeschen mit Quota-Pruefung

use std::sync::Arc;

use sha2::{Digest, Sha256};
use uuid::Uuid;

use speakeasy_db::{
    models::{NeueDatei, NachrichtenTyp as DbNachrichtenTyp, NeueNachricht},
    ChatMessageRepository, FileRepository,
};

use crate::{
    error::{ChatError, ChatResult},
    storage::StorageBackend,
    types::{ChatNachricht, DateeiInfo, DateiUpload, NachrichtenTyp},
};

/// Standard-Gruppen-ID fuer Quota-Tracking wenn keine Gruppe angegeben
const DEFAULT_GROUP: &str = "default";

/// FileService verwaltet Datei-Uploads, Downloads und Loeschungen
pub struct FileService<F, C, S>
where
    F: FileRepository,
    C: ChatMessageRepository,
    S: StorageBackend,
{
    file_repo: Arc<F>,
    chat_repo: Arc<C>,
    storage: Arc<S>,
}

impl<F, C, S> FileService<F, C, S>
where
    F: FileRepository,
    C: ChatMessageRepository,
    S: StorageBackend,
{
    /// Neuen FileService erstellen
    pub fn neu(file_repo: Arc<F>, chat_repo: Arc<C>, storage: Arc<S>) -> Arc<Self> {
        Arc::new(Self {
            file_repo,
            chat_repo,
            storage,
        })
    }

    /// Datei hochladen und als Nachricht im Kanal posten
    ///
    /// Prueft Kontingent, berechnet SHA-256, speichert Datei und legt
    /// einen Datenbank-Eintrag sowie eine Chat-Nachricht an.
    pub async fn datei_hochladen(
        &self,
        upload: DateiUpload,
        group_id: Option<&str>,
    ) -> ChatResult<(DateeiInfo, ChatNachricht)> {
        let group = group_id.unwrap_or(DEFAULT_GROUP);
        let size = upload.data.len() as i64;

        if upload.filename.trim().is_empty() {
            return Err(ChatError::UngueltigeEingabe(
                "Dateiname darf nicht leer sein".into(),
            ));
        }

        // Kontingent pruefen
        let quota = self.file_repo.get_quota(group).await?;
        if size > quota.max_file_size {
            return Err(ChatError::DateiZuGross {
                size,
                max: quota.max_file_size,
            });
        }
        if quota.current_usage + size > quota.max_total_storage {
            return Err(ChatError::KontingentErschoepft {
                used: quota.current_usage,
                max: quota.max_total_storage,
            });
        }

        // SHA-256 berechnen
        let mut hasher = Sha256::new();
        hasher.update(&upload.data);
        let checksum = format!("{:x}", hasher.finalize());

        // Speicher-Pfad aufbauen: channel_id/file_id_filename
        let file_id = Uuid::new_v4();
        let storage_path = format!(
            "{}/{}_{}",
            upload.channel_id, file_id, upload.filename
        );

        // Datei im Storage ablegen
        self.storage.store(&storage_path, &upload.data).await?;

        // DB-Eintrag anlegen
        let datei_record = self
            .file_repo
            .create(NeueDatei {
                channel_id: upload.channel_id,
                uploader_id: upload.uploader_id,
                filename: &upload.filename,
                mime_type: &upload.mime_type,
                size_bytes: size,
                storage_path: &storage_path,
                checksum: &checksum,
            })
            .await
            .map_err(|e| {
                // Bei DB-Fehler versuchen, die gespeicherte Datei wieder zu loeschen
                tracing::error!(%e, "DB-Eintrag fuer Datei fehlgeschlagen, loesche Storage-Datei");
                e
            })?;

        // Kontingent erhoehen
        self.file_repo.increment_usage(group, size).await?;

        let datei_info = DateeiInfo {
            id: datei_record.id,
            filename: datei_record.filename.clone(),
            mime_type: datei_record.mime_type.clone(),
            size_bytes: datei_record.size_bytes,
        };

        // Chat-Nachricht vom Typ 'file' erstellen
        let nachricht_content = format!("{}:{}", datei_record.id, datei_record.filename);
        let nachricht_record = self
            .chat_repo
            .create(NeueNachricht {
                channel_id: upload.channel_id,
                sender_id: upload.uploader_id,
                content: &nachricht_content,
                message_type: DbNachrichtenTyp::File,
                reply_to: None,
            })
            .await?;

        let nachricht = ChatNachricht {
            id: nachricht_record.id,
            channel_id: nachricht_record.channel_id,
            sender_id: nachricht_record.sender_id,
            content: nachricht_record.content,
            message_type: NachrichtenTyp::File,
            reply_to: None,
            file_info: Some(datei_info.clone()),
            created_at: nachricht_record.created_at,
            edited_at: None,
        };

        tracing::info!(
            file_id = %datei_record.id,
            filename = %datei_record.filename,
            size = size,
            "Datei hochgeladen"
        );

        Ok((datei_info, nachricht))
    }

    /// Datei herunterladen
    ///
    /// Gibt Datei-Metadaten und Rohdaten zurueck.
    pub async fn datei_herunterladen(
        &self,
        file_id: Uuid,
    ) -> ChatResult<(DateeiInfo, Vec<u8>)> {
        let record = self
            .file_repo
            .get_by_id(file_id)
            .await?
            .ok_or_else(|| ChatError::DateiNichtGefunden(file_id.to_string()))?;

        if record.deleted_at.is_some() {
            return Err(ChatError::DateiNichtGefunden(file_id.to_string()));
        }

        let data = self.storage.retrieve(&record.storage_path).await?;

        let info = DateeiInfo {
            id: record.id,
            filename: record.filename,
            mime_type: record.mime_type,
            size_bytes: record.size_bytes,
        };

        Ok((info, data))
    }

    /// Datei loeschen (Soft-Delete in DB + Storage-Datei entfernen)
    pub async fn datei_loeschen(
        &self,
        file_id: Uuid,
        requester_id: Uuid,
        group_id: Option<&str>,
    ) -> ChatResult<()> {
        let record = self
            .file_repo
            .get_by_id(file_id)
            .await?
            .ok_or_else(|| ChatError::DateiNichtGefunden(file_id.to_string()))?;

        if record.deleted_at.is_some() {
            return Err(ChatError::DateiNichtGefunden(file_id.to_string()));
        }

        if record.uploader_id != requester_id {
            return Err(ChatError::KeineBerechtigung(
                "Nur der Hochlader kann die Datei loeschen".into(),
            ));
        }

        // Soft-Delete in DB
        self.file_repo.soft_delete(file_id).await?;

        // Kontingent verringern
        let group = group_id.unwrap_or(DEFAULT_GROUP);
        self.file_repo
            .decrement_usage(group, record.size_bytes)
            .await?;

        // Storage-Datei loeschen (Fehler nur loggen, nicht weiterwerfen)
        if let Err(e) = self.storage.delete(&record.storage_path).await {
            tracing::warn!(%e, path = %record.storage_path, "Storage-Datei konnte nicht geloescht werden");
        }

        tracing::info!(
            file_id = %file_id,
            filename = %record.filename,
            "Datei geloescht"
        );

        Ok(())
    }

    /// Alle aktiven Dateien eines Kanals auflisten
    pub async fn dateien_auflisten(
        &self,
        channel_id: Uuid,
    ) -> ChatResult<Vec<DateeiInfo>> {
        let records = self.file_repo.list_by_channel(channel_id).await?;

        Ok(records
            .into_iter()
            .map(|r| DateeiInfo {
                id: r.id,
                filename: r.filename,
                mime_type: r.mime_type,
                size_bytes: r.size_bytes,
            })
            .collect())
    }
}
