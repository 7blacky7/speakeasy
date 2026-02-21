//! SQLite-Implementierung des AuditLogRepository

use chrono::Utc;
use sqlx::Row;
use uuid::Uuid;

use crate::error::DbError;
use crate::models::{AuditLogFilter, AuditLogRecord};
use crate::repository::{AuditLogRepository, DbResult};
use crate::sqlite::bans::parse_opt_uuid;
use crate::sqlite::pool::SqliteDb;

impl AuditLogRepository for SqliteDb {
    async fn log_event(
        &self,
        actor_id: Option<Uuid>,
        action: &str,
        target_type: Option<&str>,
        target_id: Option<&str>,
        details: serde_json::Value,
    ) -> DbResult<AuditLogRecord> {
        let id = Uuid::new_v4();
        let id_str = id.to_string();
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let actor_str = actor_id.map(|u| u.to_string());
        let details_str = serde_json::to_string(&details)?;

        sqlx::query(
            "INSERT INTO audit_log
               (id, actor_id, action, target_type, target_id, details_json, timestamp)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id_str)
        .bind(&actor_str)
        .bind(action)
        .bind(target_type)
        .bind(target_id)
        .bind(&details_str)
        .bind(&now_str)
        .execute(&self.pool)
        .await?;

        Ok(AuditLogRecord {
            id,
            actor_id,
            action: action.to_string(),
            target_type: target_type.map(|s| s.to_string()),
            target_id: target_id.map(|s| s.to_string()),
            details,
            timestamp: now,
        })
    }

    async fn list_events(&self, filter: AuditLogFilter) -> DbResult<Vec<AuditLogRecord>> {
        // Dynamische WHERE-Klausel aufbauen
        let mut conditions: Vec<&str> = Vec::new();
        let actor_str = filter.actor_id.map(|u| u.to_string());

        if actor_str.is_some() {
            conditions.push("actor_id = ?");
        }
        if filter.action.is_some() {
            conditions.push("action = ?");
        }
        if filter.target_type.is_some() {
            conditions.push("target_type = ?");
        }
        if filter.target_id.is_some() {
            conditions.push("target_id = ?");
        }
        if filter.since.is_some() {
            conditions.push("timestamp >= ?");
        }
        if filter.until.is_some() {
            conditions.push("timestamp <= ?");
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let limit_clause = filter
            .limit
            .map(|l| format!("LIMIT {l}"))
            .unwrap_or_default();
        let offset_clause = filter
            .offset
            .map(|o| format!("OFFSET {o}"))
            .unwrap_or_default();

        let sql = format!(
            "SELECT id, actor_id, action, target_type, target_id, details_json, timestamp
             FROM audit_log
             {where_clause}
             ORDER BY timestamp DESC
             {limit_clause} {offset_clause}"
        );

        let mut q = sqlx::query(&sql);

        if let Some(ref v) = actor_str {
            q = q.bind(v);
        }
        if let Some(ref v) = filter.action {
            q = q.bind(v);
        }
        if let Some(ref v) = filter.target_type {
            q = q.bind(v);
        }
        if let Some(ref v) = filter.target_id {
            q = q.bind(v);
        }
        if let Some(ref v) = filter.since {
            q = q.bind(v.to_rfc3339());
        }
        if let Some(ref v) = filter.until {
            q = q.bind(v.to_rfc3339());
        }

        let rows = q.fetch_all(&self.pool).await?;
        rows.iter().map(|r| row_to_audit(r)).collect()
    }

    async fn count_events(&self, filter: AuditLogFilter) -> DbResult<i64> {
        let actor_str = filter.actor_id.map(|u| u.to_string());
        let mut conditions: Vec<&str> = Vec::new();

        if actor_str.is_some() {
            conditions.push("actor_id = ?");
        }
        if filter.action.is_some() {
            conditions.push("action = ?");
        }
        if filter.target_type.is_some() {
            conditions.push("target_type = ?");
        }
        if filter.target_id.is_some() {
            conditions.push("target_id = ?");
        }
        if filter.since.is_some() {
            conditions.push("timestamp >= ?");
        }
        if filter.until.is_some() {
            conditions.push("timestamp <= ?");
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let sql = format!("SELECT COUNT(*) as cnt FROM audit_log {where_clause}");
        let mut q = sqlx::query(&sql);

        if let Some(ref v) = actor_str {
            q = q.bind(v);
        }
        if let Some(ref v) = filter.action {
            q = q.bind(v);
        }
        if let Some(ref v) = filter.target_type {
            q = q.bind(v);
        }
        if let Some(ref v) = filter.target_id {
            q = q.bind(v);
        }
        if let Some(ref v) = filter.since {
            q = q.bind(v.to_rfc3339());
        }
        if let Some(ref v) = filter.until {
            q = q.bind(v.to_rfc3339());
        }

        let row = q.fetch_one(&self.pool).await?;
        Ok(row.try_get("cnt")?)
    }
}

fn row_to_audit(row: &sqlx::sqlite::SqliteRow) -> DbResult<AuditLogRecord> {
    let id_str: String = row.try_get("id")?;
    let id = Uuid::parse_str(&id_str)
        .map_err(|e| DbError::intern(format!("Ungueltige AuditLog-UUID '{id_str}': {e}")))?;

    let actor_id = parse_opt_uuid(row, "actor_id")?;

    let ts_str: String = row.try_get("timestamp")?;
    let timestamp = chrono::DateTime::parse_from_rfc3339(&ts_str)
        .map_err(|e| DbError::intern(format!("Ungueltige timestamp '{ts_str}': {e}")))?
        .with_timezone(&Utc);

    let details_str: String = row.try_get("details_json")?;
    let details: serde_json::Value = serde_json::from_str(&details_str)
        .map_err(|e| DbError::intern(format!("Ungueltige details JSON: {e}")))?;

    Ok(AuditLogRecord {
        id,
        actor_id,
        action: row.try_get("action")?,
        target_type: row.try_get("target_type")?,
        target_id: row.try_get("target_id")?,
        details,
        timestamp,
    })
}
