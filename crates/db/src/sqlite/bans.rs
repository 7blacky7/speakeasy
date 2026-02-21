//! SQLite-Implementierung des BanRepository

use chrono::Utc;
use sqlx::Row;
use uuid::Uuid;

use crate::error::DbError;
use crate::models::{BanRecord, NeuerBan};
use crate::repository::{BanRepository, DbResult};
use crate::sqlite::pool::SqliteDb;

impl BanRepository for SqliteDb {
    async fn create(&self, data: NeuerBan<'_>) -> DbResult<BanRecord> {
        let id = Uuid::new_v4();
        let id_str = id.to_string();
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let user_id_str = data.user_id.map(|u| u.to_string());
        let banned_by_str = data.banned_by.map(|u| u.to_string());
        let expires_str = data.expires_at.as_ref().map(|dt| dt.to_rfc3339());

        sqlx::query(
            "INSERT INTO bans (id, user_id, ip, reason, banned_by, expires_at, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id_str)
        .bind(&user_id_str)
        .bind(data.ip)
        .bind(data.reason)
        .bind(&banned_by_str)
        .bind(&expires_str)
        .bind(&now_str)
        .execute(&self.pool)
        .await?;

        Ok(BanRecord {
            id,
            user_id: data.user_id,
            ip: data.ip.map(|s| s.to_string()),
            reason: data.reason.to_string(),
            banned_by: data.banned_by,
            expires_at: data.expires_at,
            created_at: now,
        })
    }

    async fn get(&self, id: Uuid) -> DbResult<Option<BanRecord>> {
        let row = sqlx::query(
            "SELECT id, user_id, ip, reason, banned_by, expires_at, created_at
             FROM bans WHERE id = ?",
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        row.map(|r| row_to_ban(&r)).transpose()
    }

    async fn list(&self, nur_aktive: bool) -> DbResult<Vec<BanRecord>> {
        let sql = if nur_aktive {
            "SELECT id, user_id, ip, reason, banned_by, expires_at, created_at
             FROM bans
             WHERE expires_at IS NULL OR expires_at > strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
             ORDER BY created_at DESC"
        } else {
            "SELECT id, user_id, ip, reason, banned_by, expires_at, created_at
             FROM bans ORDER BY created_at DESC"
        };

        let rows = sqlx::query(sql).fetch_all(&self.pool).await?;
        rows.iter().map(|r| row_to_ban(r)).collect()
    }

    async fn remove(&self, id: Uuid) -> DbResult<bool> {
        let affected = sqlx::query("DELETE FROM bans WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?
            .rows_affected();
        Ok(affected > 0)
    }

    async fn is_banned(
        &self,
        user_id: Option<Uuid>,
        ip: Option<&str>,
    ) -> DbResult<Option<BanRecord>> {
        // Prueft ob user_id ODER ip gebannt ist (aktive Bans)
        let user_id_str = user_id.map(|u| u.to_string());

        let row = sqlx::query(
            "SELECT id, user_id, ip, reason, banned_by, expires_at, created_at
             FROM bans
             WHERE (expires_at IS NULL OR expires_at > strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
               AND (
                 (user_id = ? AND ? IS NOT NULL)
                 OR (ip = ? AND ? IS NOT NULL)
               )
             LIMIT 1",
        )
        .bind(&user_id_str)
        .bind(&user_id_str)
        .bind(ip)
        .bind(ip)
        .fetch_optional(&self.pool)
        .await?;

        row.map(|r| row_to_ban(&r)).transpose()
    }

    async fn cleanup_expired(&self) -> DbResult<u64> {
        let affected =
            sqlx::query("DELETE FROM bans WHERE expires_at IS NOT NULL AND expires_at <= strftime('%Y-%m-%dT%H:%M:%SZ', 'now')")
                .execute(&self.pool)
                .await?
                .rows_affected();
        Ok(affected)
    }
}

fn row_to_ban(row: &sqlx::sqlite::SqliteRow) -> DbResult<BanRecord> {
    let id_str: String = row.try_get("id")?;
    let id = Uuid::parse_str(&id_str)
        .map_err(|e| DbError::intern(format!("Ungueltige Ban-UUID '{id_str}': {e}")))?;

    let user_id = parse_opt_uuid(row, "user_id")?;
    let banned_by = parse_opt_uuid(row, "banned_by")?;

    let created_at_str: String = row.try_get("created_at")?;
    let created_at = chrono::DateTime::parse_from_rfc3339(&created_at_str)
        .map_err(|e| DbError::intern(format!("Ungueltige created_at: {e}")))?
        .with_timezone(&Utc);

    let expires_at = parse_opt_datetime(row, "expires_at")?;

    Ok(BanRecord {
        id,
        user_id,
        ip: row.try_get("ip")?,
        reason: row.try_get("reason")?,
        banned_by,
        expires_at,
        created_at,
    })
}

pub(crate) fn parse_opt_uuid(
    row: &sqlx::sqlite::SqliteRow,
    col: &str,
) -> DbResult<Option<Uuid>> {
    let s: Option<String> = row.try_get(col)?;
    s.as_deref()
        .map(|v| {
            Uuid::parse_str(v)
                .map_err(|e| DbError::intern(format!("Ungueltige UUID in '{col}': {e}")))
        })
        .transpose()
}

pub(crate) fn parse_opt_datetime(
    row: &sqlx::sqlite::SqliteRow,
    col: &str,
) -> DbResult<Option<chrono::DateTime<Utc>>> {
    let s: Option<String> = row.try_get(col)?;
    s.as_deref()
        .map(|v| {
            chrono::DateTime::parse_from_rfc3339(v)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|e| DbError::intern(format!("Ungueltige DateTime in '{col}': {e}")))
        })
        .transpose()
}
