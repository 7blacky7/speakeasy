//! SQLite-Implementierung des InviteRepository

use chrono::Utc;
use sqlx::Row;
use uuid::Uuid;

use crate::error::DbError;
use crate::models::{EinladungRecord, NeueEinladung};
use crate::repository::{DbResult, InviteRepository};
use crate::sqlite::bans::{parse_opt_datetime, parse_opt_uuid};
use crate::sqlite::pool::SqliteDb;

impl InviteRepository for SqliteDb {
    async fn create(&self, data: NeueEinladung<'_>) -> DbResult<EinladungRecord> {
        let id = Uuid::new_v4();
        let id_str = id.to_string();
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let channel_id_str = data.channel_id.map(|u| u.to_string());
        let group_id_str = data.assigned_group_id.map(|u| u.to_string());
        let expires_str = data.expires_at.as_ref().map(|dt| dt.to_rfc3339());
        let created_by_str = data.created_by.to_string();

        sqlx::query(
            "INSERT INTO invites
               (id, code, channel_id, assigned_group_id, max_uses, used_count, expires_at, created_by, created_at)
             VALUES (?, ?, ?, ?, ?, 0, ?, ?, ?)",
        )
        .bind(&id_str)
        .bind(data.code)
        .bind(&channel_id_str)
        .bind(&group_id_str)
        .bind(data.max_uses)
        .bind(&expires_str)
        .bind(&created_by_str)
        .bind(&now_str)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("UNIQUE") || msg.contains("unique") {
                DbError::Eindeutigkeit(format!("Einladungscode '{}' bereits vergeben", data.code))
            } else {
                DbError::Sqlx(e)
            }
        })?;

        Ok(EinladungRecord {
            id,
            code: data.code.to_string(),
            channel_id: data.channel_id,
            assigned_group_id: data.assigned_group_id,
            max_uses: data.max_uses,
            used_count: 0,
            expires_at: data.expires_at,
            created_by: data.created_by,
            created_at: now,
        })
    }

    async fn get(&self, id: Uuid) -> DbResult<Option<EinladungRecord>> {
        let row = sqlx::query(
            "SELECT id, code, channel_id, assigned_group_id, max_uses, used_count,
                    expires_at, created_by, created_at
             FROM invites WHERE id = ?",
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        row.map(|r| row_to_invite(&r)).transpose()
    }

    async fn get_by_code(&self, code: &str) -> DbResult<Option<EinladungRecord>> {
        let row = sqlx::query(
            "SELECT id, code, channel_id, assigned_group_id, max_uses, used_count,
                    expires_at, created_by, created_at
             FROM invites WHERE code = ?",
        )
        .bind(code)
        .fetch_optional(&self.pool)
        .await?;

        row.map(|r| row_to_invite(&r)).transpose()
    }

    async fn list(&self, created_by: Option<Uuid>) -> DbResult<Vec<EinladungRecord>> {
        let rows = if let Some(uid) = created_by {
            sqlx::query(
                "SELECT id, code, channel_id, assigned_group_id, max_uses, used_count,
                         expires_at, created_by, created_at
                  FROM invites WHERE created_by = ?
                  ORDER BY created_at DESC",
            )
            .bind(uid.to_string())
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                "SELECT id, code, channel_id, assigned_group_id, max_uses, used_count,
                         expires_at, created_by, created_at
                  FROM invites ORDER BY created_at DESC",
            )
            .fetch_all(&self.pool)
            .await?
        };

        rows.iter().map(|r| row_to_invite(r)).collect()
    }

    async fn use_invite(&self, code: &str) -> DbResult<Option<EinladungRecord>> {
        // Lade und pruefe Gueltigkeit in einer Transaktion
        let mut tx = self.pool.begin().await?;

        let row = sqlx::query(
            "SELECT id, code, channel_id, assigned_group_id, max_uses, used_count,
                    expires_at, created_by, created_at
             FROM invites WHERE code = ?",
        )
        .bind(code)
        .fetch_optional(&mut *tx)
        .await?;

        let row = match row {
            None => return Ok(None),
            Some(r) => r,
        };

        let invite = row_to_invite(&row)?;

        // Ablauf pruefen
        if let Some(expires) = invite.expires_at {
            if expires < Utc::now() {
                tx.rollback().await?;
                return Err(DbError::EinladungUngueltig);
            }
        }

        // Verbrauchslimit pruefen (0 = unbegrenzt)
        if invite.max_uses > 0 && invite.used_count >= invite.max_uses {
            tx.rollback().await?;
            return Err(DbError::EinladungErschoepft);
        }

        // used_count erhoehen
        sqlx::query("UPDATE invites SET used_count = used_count + 1 WHERE code = ?")
            .bind(code)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;

        // Aktuellen Stand laden
        self.get_by_code(code).await
    }

    async fn revoke(&self, id: Uuid) -> DbResult<bool> {
        let affected = sqlx::query("DELETE FROM invites WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?
            .rows_affected();
        Ok(affected > 0)
    }
}

fn row_to_invite(row: &sqlx::sqlite::SqliteRow) -> DbResult<EinladungRecord> {
    let id_str: String = row.try_get("id")?;
    let id = Uuid::parse_str(&id_str)
        .map_err(|e| DbError::intern(format!("Ungueltige Invite-UUID '{id_str}': {e}")))?;

    let created_by_str: String = row.try_get("created_by")?;
    let created_by = Uuid::parse_str(&created_by_str)
        .map_err(|e| DbError::intern(format!("Ungueltige created_by UUID: {e}")))?;

    let created_at_str: String = row.try_get("created_at")?;
    let created_at = chrono::DateTime::parse_from_rfc3339(&created_at_str)
        .map_err(|e| DbError::intern(format!("Ungueltige created_at: {e}")))?
        .with_timezone(&Utc);

    let channel_id = parse_opt_uuid(row, "channel_id")?;
    let assigned_group_id = parse_opt_uuid(row, "assigned_group_id")?;
    let expires_at = parse_opt_datetime(row, "expires_at")?;

    Ok(EinladungRecord {
        id,
        code: row.try_get("code")?,
        channel_id,
        assigned_group_id,
        max_uses: row.try_get("max_uses")?,
        used_count: row.try_get("used_count")?,
        expires_at,
        created_by,
        created_at,
    })
}
