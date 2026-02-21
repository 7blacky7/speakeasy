//! SQLite-Implementierung des FileRepository

use chrono::Utc;
use uuid::Uuid;

use crate::error::DbError;
use crate::models::{DateiKontingentRecord, DateiRecord, NeueDatei};
use crate::repository::{DbResult, FileRepository};
use crate::sqlite::pool::SqliteDb;

impl FileRepository for SqliteDb {
    async fn create(&self, data: NeueDatei<'_>) -> DbResult<DateiRecord> {
        let id = Uuid::new_v4();
        let id_str = id.to_string();
        let channel_str = data.channel_id.to_string();
        let uploader_str = data.uploader_id.to_string();
        let now = Utc::now();
        let now_str = now.format("%Y-%m-%dT%H:%M:%SZ").to_string();

        sqlx::query(
            "INSERT INTO files
             (id, channel_id, uploader_id, filename, mime_type, size_bytes, storage_path, checksum, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id_str)
        .bind(&channel_str)
        .bind(&uploader_str)
        .bind(data.filename)
        .bind(data.mime_type)
        .bind(data.size_bytes)
        .bind(data.storage_path)
        .bind(data.checksum)
        .bind(&now_str)
        .execute(&self.pool)
        .await?;

        Ok(DateiRecord {
            id,
            channel_id: data.channel_id,
            uploader_id: data.uploader_id,
            filename: data.filename.to_string(),
            mime_type: data.mime_type.to_string(),
            size_bytes: data.size_bytes,
            storage_path: data.storage_path.to_string(),
            checksum: data.checksum.to_string(),
            created_at: now,
            deleted_at: None,
        })
    }

    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<DateiRecord>> {
        let row = sqlx::query(
            "SELECT id, channel_id, uploader_id, filename, mime_type,
                    size_bytes, storage_path, checksum, created_at, deleted_at
             FROM files WHERE id = ?",
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        row.map(|r| row_to_datei(&r)).transpose()
    }

    async fn list_by_channel(&self, channel_id: Uuid) -> DbResult<Vec<DateiRecord>> {
        let rows = sqlx::query(
            "SELECT id, channel_id, uploader_id, filename, mime_type,
                    size_bytes, storage_path, checksum, created_at, deleted_at
             FROM files
             WHERE channel_id = ? AND deleted_at IS NULL
             ORDER BY created_at DESC",
        )
        .bind(channel_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(|r| row_to_datei(r)).collect()
    }

    async fn soft_delete(&self, id: Uuid) -> DbResult<bool> {
        let now_str = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

        let affected =
            sqlx::query("UPDATE files SET deleted_at = ? WHERE id = ? AND deleted_at IS NULL")
                .bind(&now_str)
                .bind(id.to_string())
                .execute(&self.pool)
                .await?
                .rows_affected();

        Ok(affected > 0)
    }

    async fn get_quota(&self, group_id: &str) -> DbResult<DateiKontingentRecord> {
        let row = sqlx::query(
            "SELECT group_id, max_file_size, max_total_storage, current_usage
             FROM file_quotas WHERE group_id = ?",
        )
        .bind(group_id)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(r) = row {
            use sqlx::Row as _;
            Ok(DateiKontingentRecord {
                group_id: r.try_get("group_id")?,
                max_file_size: r.try_get("max_file_size")?,
                max_total_storage: r.try_get("max_total_storage")?,
                current_usage: r.try_get("current_usage")?,
            })
        } else {
            // Standard-Kontingent zurueckgeben wenn noch keines konfiguriert
            Ok(DateiKontingentRecord {
                group_id: group_id.to_string(),
                max_file_size: 10 * 1024 * 1024,       // 10 MB
                max_total_storage: 1024 * 1024 * 1024, // 1 GB
                current_usage: 0,
            })
        }
    }

    async fn increment_usage(&self, group_id: &str, bytes: i64) -> DbResult<()> {
        // Upsert: Wenn kein Eintrag vorhanden, Standard-Werte einfuegen
        sqlx::query(
            "INSERT INTO file_quotas (group_id, max_file_size, max_total_storage, current_usage)
             VALUES (?, 10485760, 1073741824, ?)
             ON CONFLICT(group_id) DO UPDATE SET current_usage = current_usage + ?",
        )
        .bind(group_id)
        .bind(bytes)
        .bind(bytes)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn decrement_usage(&self, group_id: &str, bytes: i64) -> DbResult<()> {
        sqlx::query(
            "UPDATE file_quotas
             SET current_usage = MAX(0, current_usage - ?)
             WHERE group_id = ?",
        )
        .bind(bytes)
        .bind(group_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

pub(crate) fn row_to_datei(row: &sqlx::sqlite::SqliteRow) -> DbResult<DateiRecord> {
    use sqlx::Row as _;

    let id_str: String = row.try_get("id")?;
    let id = Uuid::parse_str(&id_str)
        .map_err(|e| DbError::intern(format!("Ungueltige Datei-UUID '{id_str}': {e}")))?;

    let channel_str: String = row.try_get("channel_id")?;
    let channel_id = Uuid::parse_str(&channel_str)
        .map_err(|e| DbError::intern(format!("Ungueltige channel_id UUID '{channel_str}': {e}")))?;

    let uploader_str: String = row.try_get("uploader_id")?;
    let uploader_id = Uuid::parse_str(&uploader_str).map_err(|e| {
        DbError::intern(format!("Ungueltige uploader_id UUID '{uploader_str}': {e}"))
    })?;

    let created_at = parse_db_timestamp(row.try_get("created_at")?)?;
    let deleted_at: Option<String> = row.try_get("deleted_at")?;
    let deleted_at = deleted_at.map(parse_db_timestamp).transpose()?;

    Ok(DateiRecord {
        id,
        channel_id,
        uploader_id,
        filename: row.try_get("filename")?,
        mime_type: row.try_get("mime_type")?,
        size_bytes: row.try_get("size_bytes")?,
        storage_path: row.try_get("storage_path")?,
        checksum: row.try_get("checksum")?,
        created_at,
        deleted_at,
    })
}

fn parse_db_timestamp(s: String) -> DbResult<chrono::DateTime<Utc>> {
    chrono::DateTime::parse_from_rfc3339(&s)
        .or_else(|_| {
            chrono::NaiveDateTime::parse_from_str(&s, "%Y-%m-%dT%H:%M:%SZ")
                .map(|ndt| ndt.and_utc().fixed_offset())
                .or_else(|_| {
                    chrono::NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H:%M:%S")
                        .map(|ndt| ndt.and_utc().fixed_offset())
                })
        })
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| DbError::intern(format!("Ungueltige Zeitangabe '{s}': {e}")))
}
