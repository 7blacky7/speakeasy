//! SQLite-Implementierung des UserRepository

use chrono::Utc;
use uuid::Uuid;

use crate::error::DbError;
use crate::models::{BenutzerRecord, BenutzerUpdate, NeuerBenutzer};
use crate::repository::{DbResult, UserRepository};
use crate::sqlite::pool::SqliteDb;

impl UserRepository for SqliteDb {
    async fn create(&self, data: NeuerBenutzer<'_>) -> DbResult<BenutzerRecord> {
        let id = Uuid::new_v4();
        let id_str = id.to_string();
        let now = Utc::now();
        let now_str = now.to_rfc3339();

        sqlx::query(
            "INSERT INTO users (id, username, password_hash, created_at, is_active, password_changed)
             VALUES (?, ?, ?, ?, 1, 0)",
        )
        .bind(&id_str)
        .bind(data.username)
        .bind(data.password_hash)
        .bind(&now_str)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("UNIQUE") || msg.contains("unique") {
                DbError::Eindeutigkeit(format!("Benutzername '{}' bereits vergeben", data.username))
            } else {
                DbError::Sqlx(e)
            }
        })?;

        Ok(BenutzerRecord {
            id,
            username: data.username.to_string(),
            password_hash: data.password_hash.to_string(),
            created_at: now,
            last_login: None,
            is_active: true,
            password_changed: false,
        })
    }

    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<BenutzerRecord>> {
        let row = sqlx::query(
            "SELECT id, username, password_hash, created_at, last_login, is_active, password_changed
             FROM users WHERE id = ?",
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        row.map(|r| row_to_benutzer(&r)).transpose()
    }

    async fn get_by_name(&self, username: &str) -> DbResult<Option<BenutzerRecord>> {
        let row = sqlx::query(
            "SELECT id, username, password_hash, created_at, last_login, is_active, password_changed
             FROM users WHERE username = ?",
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await?;

        row.map(|r| row_to_benutzer(&r)).transpose()
    }

    async fn update(&self, id: Uuid, data: BenutzerUpdate) -> DbResult<BenutzerRecord> {
        // Dynamisches UPDATE – nur gesetzte Felder aendern
        let mut sets: Vec<&str> = Vec::new();
        if data.username.is_some() {
            sets.push("username = ?");
        }
        if data.password_hash.is_some() {
            sets.push("password_hash = ?");
        }
        if data.is_active.is_some() {
            sets.push("is_active = ?");
        }
        if data.last_login.is_some() {
            sets.push("last_login = ?");
        }
        if data.password_changed.is_some() {
            sets.push("password_changed = ?");
        }

        if sets.is_empty() {
            return self
                .get_by_id(id)
                .await?
                .ok_or_else(|| DbError::nicht_gefunden(format!("User {id}")));
        }

        let sql = format!("UPDATE users SET {} WHERE id = ?", sets.join(", "));
        let mut q = sqlx::query(&sql);

        if let Some(ref v) = data.username {
            q = q.bind(v);
        }
        if let Some(ref v) = data.password_hash {
            q = q.bind(v);
        }
        if let Some(v) = data.is_active {
            q = q.bind(v as i64);
        }
        if let Some(ref v) = data.last_login {
            q = q.bind(v.to_rfc3339());
        }
        if let Some(v) = data.password_changed {
            q = q.bind(v as i64);
        }
        q = q.bind(id.to_string());

        let affected = q.execute(&self.pool).await?.rows_affected();
        if affected == 0 {
            return Err(DbError::nicht_gefunden(format!("User {id}")));
        }

        self.get_by_id(id)
            .await?
            .ok_or_else(|| DbError::intern("User nach Update nicht gefunden"))
    }

    async fn delete(&self, id: Uuid) -> DbResult<bool> {
        // Weicher Loeschvorgang: is_active = 0
        let affected = sqlx::query("UPDATE users SET is_active = 0 WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?
            .rows_affected();
        Ok(affected > 0)
    }

    async fn list(&self, nur_aktive: bool) -> DbResult<Vec<BenutzerRecord>> {
        let sql = if nur_aktive {
            "SELECT id, username, password_hash, created_at, last_login, is_active, password_changed
             FROM users WHERE is_active = 1 ORDER BY username"
        } else {
            "SELECT id, username, password_hash, created_at, last_login, is_active, password_changed
             FROM users ORDER BY username"
        };

        let rows = sqlx::query(sql).fetch_all(&self.pool).await?;

        rows.iter().map(row_to_benutzer).collect()
    }

    async fn authenticate(
        &self,
        username: &str,
        password_hash: &str,
    ) -> DbResult<Option<BenutzerRecord>> {
        let row = sqlx::query(
            "SELECT id, username, password_hash, created_at, last_login, is_active, password_changed
             FROM users
             WHERE username = ? AND password_hash = ? AND is_active = 1",
        )
        .bind(username)
        .bind(password_hash)
        .fetch_optional(&self.pool)
        .await?;

        row.map(|r| row_to_benutzer(&r)).transpose()
    }

    async fn update_last_login(&self, id: Uuid) -> DbResult<()> {
        let now = Utc::now().to_rfc3339();
        sqlx::query("UPDATE users SET last_login = ? WHERE id = ?")
            .bind(&now)
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

fn row_to_benutzer(row: &sqlx::sqlite::SqliteRow) -> DbResult<BenutzerRecord> {
    use sqlx::Row as _;

    let id_str: String = row.try_get("id")?;
    let id = Uuid::parse_str(&id_str)
        .map_err(|e| DbError::intern(format!("Ungueltige UUID '{id_str}': {e}")))?;

    let created_at_str: String = row.try_get("created_at")?;
    let created_at = chrono::DateTime::parse_from_rfc3339(&created_at_str)
        .map_err(|e| DbError::intern(format!("Ungueltige created_at '{created_at_str}': {e}")))?
        .with_timezone(&Utc);

    let last_login: Option<String> = row.try_get("last_login")?;
    let last_login = last_login
        .as_deref()
        .map(|s| {
            chrono::DateTime::parse_from_rfc3339(s)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|e| DbError::intern(format!("Ungueltige last_login '{s}': {e}")))
        })
        .transpose()?;

    let is_active: i64 = row.try_get("is_active")?;
    let password_changed: i64 = row.try_get("password_changed").unwrap_or(1);

    Ok(BenutzerRecord {
        id,
        username: row.try_get("username")?,
        password_hash: row.try_get("password_hash")?,
        created_at,
        last_login,
        is_active: is_active != 0,
        password_changed: password_changed != 0,
    })
}
