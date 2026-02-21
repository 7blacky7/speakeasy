//! SQLite-Implementierung des ChannelRepository

use chrono::Utc;
use uuid::Uuid;

use crate::error::DbError;
use crate::models::{KanalRecord, KanalTyp, KanalUpdate, NeuerKanal};
use crate::repository::{ChannelRepository, DbResult};
use crate::sqlite::pool::SqliteDb;

impl ChannelRepository for SqliteDb {
    async fn create(&self, data: NeuerKanal<'_>) -> DbResult<KanalRecord> {
        let id = Uuid::new_v4();
        let id_str = id.to_string();
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let parent_str = data.parent_id.map(|u| u.to_string());

        sqlx::query(
            "INSERT INTO channels
             (id, name, parent_id, topic, password_hash, max_clients, is_default, sort_order, channel_type, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id_str)
        .bind(data.name)
        .bind(&parent_str)
        .bind(data.topic)
        .bind(data.password_hash)
        .bind(data.max_clients)
        .bind(data.is_default as i64)
        .bind(data.sort_order)
        .bind(data.channel_type.als_str())
        .bind(&now_str)
        .execute(&self.pool)
        .await?;

        Ok(KanalRecord {
            id,
            name: data.name.to_string(),
            parent_id: data.parent_id,
            topic: data.topic.map(|s| s.to_string()),
            password_hash: data.password_hash.map(|s| s.to_string()),
            max_clients: data.max_clients,
            is_default: data.is_default,
            sort_order: data.sort_order,
            channel_type: data.channel_type,
            created_at: now,
        })
    }

    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<KanalRecord>> {
        let row = sqlx::query(
            "SELECT id, name, parent_id, topic, password_hash, max_clients,
                    is_default, sort_order, channel_type, created_at
             FROM channels WHERE id = ?",
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        row.map(|r| row_to_kanal(&r)).transpose()
    }

    async fn list(&self) -> DbResult<Vec<KanalRecord>> {
        let rows = sqlx::query(
            "SELECT id, name, parent_id, topic, password_hash, max_clients,
                    is_default, sort_order, channel_type, created_at
             FROM channels ORDER BY sort_order, name",
        )
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(row_to_kanal).collect()
    }

    async fn update(&self, id: Uuid, data: KanalUpdate) -> DbResult<KanalRecord> {
        let mut sets: Vec<String> = Vec::new();

        if data.name.is_some() {
            sets.push("name = ?".into());
        }
        if data.parent_id.is_some() {
            sets.push("parent_id = ?".into());
        }
        if data.topic.is_some() {
            sets.push("topic = ?".into());
        }
        if data.password_hash.is_some() {
            sets.push("password_hash = ?".into());
        }
        if data.max_clients.is_some() {
            sets.push("max_clients = ?".into());
        }
        if data.is_default.is_some() {
            sets.push("is_default = ?".into());
        }
        if data.sort_order.is_some() {
            sets.push("sort_order = ?".into());
        }

        if sets.is_empty() {
            return self
                .get_by_id(id)
                .await?
                .ok_or_else(|| DbError::nicht_gefunden(format!("Kanal {id}")));
        }

        let sql = format!("UPDATE channels SET {} WHERE id = ?", sets.join(", "));
        let mut q = sqlx::query(&sql);

        if let Some(ref v) = data.name {
            q = q.bind(v);
        }
        if let Some(ref v) = data.parent_id {
            q = q.bind(v.map(|u| u.to_string()));
        }
        if let Some(ref v) = data.topic {
            q = q.bind(v.as_deref());
        }
        if let Some(ref v) = data.password_hash {
            q = q.bind(v.as_deref());
        }
        if let Some(v) = data.max_clients {
            q = q.bind(v);
        }
        if let Some(v) = data.is_default {
            q = q.bind(v as i64);
        }
        if let Some(v) = data.sort_order {
            q = q.bind(v);
        }
        q = q.bind(id.to_string());

        let affected = q.execute(&self.pool).await?.rows_affected();
        if affected == 0 {
            return Err(DbError::nicht_gefunden(format!("Kanal {id}")));
        }

        self.get_by_id(id)
            .await?
            .ok_or_else(|| DbError::intern("Kanal nach Update nicht gefunden"))
    }

    async fn delete(&self, id: Uuid) -> DbResult<bool> {
        let affected = sqlx::query("DELETE FROM channels WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?
            .rows_affected();
        Ok(affected > 0)
    }

    async fn get_children(&self, parent_id: Uuid) -> DbResult<Vec<KanalRecord>> {
        let rows = sqlx::query(
            "SELECT id, name, parent_id, topic, password_hash, max_clients,
                    is_default, sort_order, channel_type, created_at
             FROM channels WHERE parent_id = ?
             ORDER BY sort_order, name",
        )
        .bind(parent_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(row_to_kanal).collect()
    }

    async fn get_default(&self) -> DbResult<Option<KanalRecord>> {
        let row = sqlx::query(
            "SELECT id, name, parent_id, topic, password_hash, max_clients,
                    is_default, sort_order, channel_type, created_at
             FROM channels WHERE is_default = 1 LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await?;

        row.map(|r| row_to_kanal(&r)).transpose()
    }
}

pub(crate) fn row_to_kanal(row: &sqlx::sqlite::SqliteRow) -> DbResult<KanalRecord> {
    use sqlx::Row as _;

    let id_str: String = row.try_get("id")?;
    let id = Uuid::parse_str(&id_str)
        .map_err(|e| DbError::intern(format!("Ungueltige Kanal-UUID '{id_str}': {e}")))?;

    let parent_str: Option<String> = row.try_get("parent_id")?;
    let parent_id = parent_str
        .as_deref()
        .map(|s| {
            Uuid::parse_str(s)
                .map_err(|e| DbError::intern(format!("Ungueltige parent_id UUID '{s}': {e}")))
        })
        .transpose()?;

    let created_at_str: String = row.try_get("created_at")?;
    let created_at = chrono::DateTime::parse_from_rfc3339(&created_at_str)
        .map_err(|e| DbError::intern(format!("Ungueltige created_at '{created_at_str}': {e}")))?
        .with_timezone(&Utc);

    let typ_str: String = row.try_get("channel_type")?;
    let channel_type = typ_str
        .parse::<KanalTyp>()
        .map_err(DbError::intern)?;

    let is_default: i64 = row.try_get("is_default")?;

    Ok(KanalRecord {
        id,
        name: row.try_get("name")?,
        parent_id,
        topic: row.try_get("topic")?,
        password_hash: row.try_get("password_hash")?,
        max_clients: row.try_get("max_clients")?,
        is_default: is_default != 0,
        sort_order: row.try_get("sort_order")?,
        channel_type,
        created_at,
    })
}
