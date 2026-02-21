//! SQLite-Implementierung der Group-Repositories (Server- und Kanal-Gruppen)

use sqlx::Row;
use uuid::Uuid;

use crate::error::DbError;
use crate::models::{KanalGruppeRecord, NeueKanalGruppe, NeueServerGruppe, ServerGruppeRecord};
use crate::repository::{ChannelGroupRepository, DbResult, ServerGroupRepository};
use crate::sqlite::pool::SqliteDb;

// ---------------------------------------------------------------------------
// ServerGroupRepository
// ---------------------------------------------------------------------------

impl ServerGroupRepository for SqliteDb {
    async fn create(&self, data: NeueServerGruppe<'_>) -> DbResult<ServerGruppeRecord> {
        let id = Uuid::new_v4();
        let id_str = id.to_string();

        sqlx::query(
            "INSERT INTO server_groups (id, name, priority, is_default, permissions)
             VALUES (?, ?, ?, ?, '{}')",
        )
        .bind(&id_str)
        .bind(data.name)
        .bind(data.priority)
        .bind(data.is_default as i64)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("UNIQUE") || msg.contains("unique") {
                DbError::Eindeutigkeit(format!("Server-Gruppe '{}' existiert bereits", data.name))
            } else {
                DbError::Sqlx(e)
            }
        })?;

        Ok(ServerGruppeRecord {
            id,
            name: data.name.to_string(),
            priority: data.priority,
            is_default: data.is_default,
            permissions: serde_json::Value::Object(Default::default()),
        })
    }

    async fn get(&self, id: Uuid) -> DbResult<Option<ServerGruppeRecord>> {
        let row = sqlx::query(
            "SELECT id, name, priority, is_default, permissions
             FROM server_groups WHERE id = ?",
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        row.map(|r| row_to_server_gruppe(&r)).transpose()
    }

    async fn list(&self) -> DbResult<Vec<ServerGruppeRecord>> {
        let rows = sqlx::query(
            "SELECT id, name, priority, is_default, permissions
             FROM server_groups ORDER BY priority DESC, name",
        )
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(row_to_server_gruppe).collect()
    }

    async fn list_for_user(&self, user_id: Uuid) -> DbResult<Vec<ServerGruppeRecord>> {
        let rows = sqlx::query(
            "SELECT sg.id, sg.name, sg.priority, sg.is_default, sg.permissions
             FROM server_groups sg
             JOIN user_server_groups usg ON usg.group_id = sg.id
             WHERE usg.user_id = ?
             ORDER BY sg.priority DESC",
        )
        .bind(user_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(row_to_server_gruppe).collect()
    }

    async fn add_member(&self, group_id: Uuid, user_id: Uuid) -> DbResult<()> {
        sqlx::query(
            "INSERT OR IGNORE INTO user_server_groups (user_id, group_id) VALUES (?, ?)",
        )
        .bind(user_id.to_string())
        .bind(group_id.to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn remove_member(&self, group_id: Uuid, user_id: Uuid) -> DbResult<bool> {
        let affected =
            sqlx::query("DELETE FROM user_server_groups WHERE user_id = ? AND group_id = ?")
                .bind(user_id.to_string())
                .bind(group_id.to_string())
                .execute(&self.pool)
                .await?
                .rows_affected();
        Ok(affected > 0)
    }

    async fn get_default(&self) -> DbResult<Option<ServerGruppeRecord>> {
        let row = sqlx::query(
            "SELECT id, name, priority, is_default, permissions
             FROM server_groups WHERE is_default = 1 LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await?;

        row.map(|r| row_to_server_gruppe(&r)).transpose()
    }

    async fn delete(&self, id: Uuid) -> DbResult<bool> {
        let affected = sqlx::query("DELETE FROM server_groups WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?
            .rows_affected();
        Ok(affected > 0)
    }
}

fn row_to_server_gruppe(row: &sqlx::sqlite::SqliteRow) -> DbResult<ServerGruppeRecord> {
    let id_str: String = row.try_get("id")?;
    let id = Uuid::parse_str(&id_str)
        .map_err(|e| DbError::intern(format!("Ungueltige UUID '{id_str}': {e}")))?;

    let perm_str: String = row.try_get("permissions")?;
    let permissions: serde_json::Value = serde_json::from_str(&perm_str)
        .map_err(|e| DbError::intern(format!("Ungueltige permissions JSON: {e}")))?;

    let is_default: i64 = row.try_get("is_default")?;

    Ok(ServerGruppeRecord {
        id,
        name: row.try_get("name")?,
        priority: row.try_get("priority")?,
        is_default: is_default != 0,
        permissions,
    })
}

// ---------------------------------------------------------------------------
// ChannelGroupRepository
// ---------------------------------------------------------------------------

impl ChannelGroupRepository for SqliteDb {
    async fn create(&self, data: NeueKanalGruppe<'_>) -> DbResult<KanalGruppeRecord> {
        let id = Uuid::new_v4();
        let id_str = id.to_string();

        sqlx::query(
            "INSERT INTO channel_groups (id, name, permissions) VALUES (?, ?, '{}')",
        )
        .bind(&id_str)
        .bind(data.name)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("UNIQUE") || msg.contains("unique") {
                DbError::Eindeutigkeit(format!("Kanal-Gruppe '{}' existiert bereits", data.name))
            } else {
                DbError::Sqlx(e)
            }
        })?;

        Ok(KanalGruppeRecord {
            id,
            name: data.name.to_string(),
            permissions: serde_json::Value::Object(Default::default()),
        })
    }

    async fn get(&self, id: Uuid) -> DbResult<Option<KanalGruppeRecord>> {
        let row = sqlx::query(
            "SELECT id, name, permissions FROM channel_groups WHERE id = ?",
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        row.map(|r| row_to_kanal_gruppe(&r)).transpose()
    }

    async fn list(&self) -> DbResult<Vec<KanalGruppeRecord>> {
        let rows = sqlx::query(
            "SELECT id, name, permissions FROM channel_groups ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(row_to_kanal_gruppe).collect()
    }

    async fn get_for_user_in_channel(
        &self,
        user_id: Uuid,
        channel_id: Uuid,
    ) -> DbResult<Option<KanalGruppeRecord>> {
        let row = sqlx::query(
            "SELECT cg.id, cg.name, cg.permissions
             FROM channel_groups cg
             JOIN user_channel_groups ucg ON ucg.group_id = cg.id
             WHERE ucg.user_id = ? AND ucg.channel_id = ?",
        )
        .bind(user_id.to_string())
        .bind(channel_id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        row.map(|r| row_to_kanal_gruppe(&r)).transpose()
    }

    async fn set_member_group(
        &self,
        user_id: Uuid,
        channel_id: Uuid,
        group_id: Uuid,
    ) -> DbResult<()> {
        sqlx::query(
            "INSERT INTO user_channel_groups (user_id, channel_id, group_id)
             VALUES (?, ?, ?)
             ON CONFLICT(user_id, channel_id) DO UPDATE SET group_id = excluded.group_id",
        )
        .bind(user_id.to_string())
        .bind(channel_id.to_string())
        .bind(group_id.to_string())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn remove_member_group(&self, user_id: Uuid, channel_id: Uuid) -> DbResult<bool> {
        let affected = sqlx::query(
            "DELETE FROM user_channel_groups WHERE user_id = ? AND channel_id = ?",
        )
        .bind(user_id.to_string())
        .bind(channel_id.to_string())
        .execute(&self.pool)
        .await?
        .rows_affected();
        Ok(affected > 0)
    }

    async fn delete(&self, id: Uuid) -> DbResult<bool> {
        let affected = sqlx::query("DELETE FROM channel_groups WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?
            .rows_affected();
        Ok(affected > 0)
    }
}

fn row_to_kanal_gruppe(row: &sqlx::sqlite::SqliteRow) -> DbResult<KanalGruppeRecord> {
    let id_str: String = row.try_get("id")?;
    let id = Uuid::parse_str(&id_str)
        .map_err(|e| DbError::intern(format!("Ungueltige UUID '{id_str}': {e}")))?;

    let perm_str: String = row.try_get("permissions")?;
    let permissions: serde_json::Value = serde_json::from_str(&perm_str)
        .map_err(|e| DbError::intern(format!("Ungueltige permissions JSON: {e}")))?;

    Ok(KanalGruppeRecord {
        id,
        name: row.try_get("name")?,
        permissions,
    })
}
