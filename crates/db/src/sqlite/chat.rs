//! SQLite-Implementierung des ChatMessageRepository

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::error::DbError;
use crate::models::{ChatNachrichtRecord, NachrichtenFilter, NachrichtenTyp, NeueNachricht};
use crate::repository::{ChatMessageRepository, DbResult};
use crate::sqlite::pool::SqliteDb;

impl ChatMessageRepository for SqliteDb {
    async fn create(&self, data: NeueNachricht<'_>) -> DbResult<ChatNachrichtRecord> {
        let id = Uuid::new_v4();
        let id_str = id.to_string();
        let channel_str = data.channel_id.to_string();
        let sender_str = data.sender_id.to_string();
        let reply_str = data.reply_to.map(|u| u.to_string());
        let now = Utc::now();
        let now_str = now.format("%Y-%m-%dT%H:%M:%SZ").to_string();

        sqlx::query(
            "INSERT INTO chat_messages
             (id, channel_id, sender_id, content, message_type, reply_to, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id_str)
        .bind(&channel_str)
        .bind(&sender_str)
        .bind(data.content)
        .bind(data.message_type.als_str())
        .bind(&reply_str)
        .bind(&now_str)
        .execute(&self.pool)
        .await?;

        Ok(ChatNachrichtRecord {
            id,
            channel_id: data.channel_id,
            sender_id: data.sender_id,
            content: data.content.to_string(),
            message_type: data.message_type,
            reply_to: data.reply_to,
            created_at: now,
            edited_at: None,
            deleted_at: None,
        })
    }

    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<ChatNachrichtRecord>> {
        let row = sqlx::query(
            "SELECT id, channel_id, sender_id, content, message_type,
                    reply_to, created_at, edited_at, deleted_at
             FROM chat_messages WHERE id = ?",
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        row.map(|r| row_to_nachricht(&r)).transpose()
    }

    async fn get_history(&self, filter: NachrichtenFilter) -> DbResult<Vec<ChatNachrichtRecord>> {
        let limit = filter.limit.unwrap_or(50);
        let channel_str = filter.channel_id.to_string();

        let rows = if let Some(before) = filter.before {
            let before_str = before.format("%Y-%m-%dT%H:%M:%SZ").to_string();
            sqlx::query(
                "SELECT id, channel_id, sender_id, content, message_type,
                         reply_to, created_at, edited_at, deleted_at
                 FROM chat_messages
                 WHERE channel_id = ? AND created_at < ? AND deleted_at IS NULL
                 ORDER BY created_at DESC
                 LIMIT ?",
            )
            .bind(&channel_str)
            .bind(&before_str)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                "SELECT id, channel_id, sender_id, content, message_type,
                         reply_to, created_at, edited_at, deleted_at
                 FROM chat_messages
                 WHERE channel_id = ? AND deleted_at IS NULL
                 ORDER BY created_at DESC
                 LIMIT ?",
            )
            .bind(&channel_str)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        };

        // Chronologisch sortieren (aelteste zuerst)
        let mut records: Vec<ChatNachrichtRecord> =
            rows.iter().map(row_to_nachricht).collect::<DbResult<_>>()?;
        records.sort_by_key(|r| r.created_at);
        Ok(records)
    }

    async fn update_content(&self, id: Uuid, new_content: &str) -> DbResult<ChatNachrichtRecord> {
        let now_str = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

        let affected = sqlx::query(
            "UPDATE chat_messages SET content = ?, edited_at = ? WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(new_content)
        .bind(&now_str)
        .bind(id.to_string())
        .execute(&self.pool)
        .await?
        .rows_affected();

        if affected == 0 {
            return Err(DbError::nicht_gefunden(format!("Nachricht {id}")));
        }

        self.get_by_id(id)
            .await?
            .ok_or_else(|| DbError::intern("Nachricht nach Update nicht gefunden"))
    }

    async fn soft_delete(&self, id: Uuid) -> DbResult<bool> {
        let now_str = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

        let affected = sqlx::query(
            "UPDATE chat_messages SET deleted_at = ? WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(&now_str)
        .bind(id.to_string())
        .execute(&self.pool)
        .await?
        .rows_affected();

        Ok(affected > 0)
    }

    async fn search(
        &self,
        channel_id: Uuid,
        query: &str,
        limit: i64,
    ) -> DbResult<Vec<ChatNachrichtRecord>> {
        let pattern = format!("%{}%", query.replace('%', "\\%").replace('_', "\\_"));
        let channel_str = channel_id.to_string();

        let rows = sqlx::query(
            "SELECT id, channel_id, sender_id, content, message_type,
                    reply_to, created_at, edited_at, deleted_at
             FROM chat_messages
             WHERE channel_id = ? AND content LIKE ? ESCAPE '\\' AND deleted_at IS NULL
             ORDER BY created_at DESC
             LIMIT ?",
        )
        .bind(&channel_str)
        .bind(&pattern)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(row_to_nachricht).collect()
    }
}

pub(crate) fn row_to_nachricht(row: &sqlx::sqlite::SqliteRow) -> DbResult<ChatNachrichtRecord> {
    use sqlx::Row as _;

    let id_str: String = row.try_get("id")?;
    let id = Uuid::parse_str(&id_str)
        .map_err(|e| DbError::intern(format!("Ungueltige Nachrichten-UUID '{id_str}': {e}")))?;

    let channel_str: String = row.try_get("channel_id")?;
    let channel_id = Uuid::parse_str(&channel_str)
        .map_err(|e| DbError::intern(format!("Ungueltige channel_id UUID '{channel_str}': {e}")))?;

    let sender_str: String = row.try_get("sender_id")?;
    let sender_id = Uuid::parse_str(&sender_str)
        .map_err(|e| DbError::intern(format!("Ungueltige sender_id UUID '{sender_str}': {e}")))?;

    let reply_str: Option<String> = row.try_get("reply_to")?;
    let reply_to = reply_str
        .as_deref()
        .map(|s| {
            Uuid::parse_str(s)
                .map_err(|e| DbError::intern(format!("Ungueltige reply_to UUID '{s}': {e}")))
        })
        .transpose()?;

    let created_at = parse_timestamp(row.try_get("created_at")?)?;
    let edited_at: Option<String> = row.try_get("edited_at")?;
    let edited_at = edited_at.map(parse_timestamp).transpose()?;
    let deleted_at: Option<String> = row.try_get("deleted_at")?;
    let deleted_at = deleted_at.map(parse_timestamp).transpose()?;

    let typ_str: String = row.try_get("message_type")?;
    let message_type = typ_str.parse::<NachrichtenTyp>().map_err(DbError::intern)?;

    Ok(ChatNachrichtRecord {
        id,
        channel_id,
        sender_id,
        content: row.try_get("content")?,
        message_type,
        reply_to,
        created_at,
        edited_at,
        deleted_at,
    })
}

fn parse_timestamp(s: String) -> DbResult<DateTime<Utc>> {
    // Versuche ISO8601 / RFC3339
    chrono::DateTime::parse_from_rfc3339(&s)
        .or_else(|_| {
            // Fallback fuer SQLite datetime()-Format ohne 'T' und 'Z'
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
