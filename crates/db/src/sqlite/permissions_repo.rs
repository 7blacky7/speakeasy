//! SQLite-Implementierung des PermissionRepository

use sqlx::Row;
use uuid::Uuid;

use crate::error::DbError;
use crate::models::{BerechtigungsWert, BerechtigungsZiel, EffektiveBerechtigung, TriState};
use crate::permissions::{berechtigungen_aufloesen, BerechtigungsEingabe};
use crate::repository::{DbResult, PermissionRepository};
use crate::sqlite::pool::SqliteDb;

impl PermissionRepository for SqliteDb {
    async fn get_permissions(
        &self,
        ziel: &BerechtigungsZiel,
        channel_id: Option<Uuid>,
    ) -> DbResult<Vec<(String, BerechtigungsWert)>> {
        let target_type = ziel.typ_str();
        let target_id = ziel.id().map(|u| u.to_string());
        let ch_id = channel_id.map(|u| u.to_string());

        let rows = sqlx::query(
            "SELECT permission_key, value_type, tri_state, int_limit, scope_json
             FROM permissions
             WHERE target_type = ?
               AND (target_id = ? OR (target_id IS NULL AND ? IS NULL))
               AND (channel_id = ? OR (channel_id IS NULL AND ? IS NULL))",
        )
        .bind(target_type)
        .bind(&target_id)
        .bind(&target_id)
        .bind(&ch_id)
        .bind(&ch_id)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(row_to_permission).collect()
    }

    async fn set_permission(
        &self,
        ziel: &BerechtigungsZiel,
        permission_key: &str,
        wert: BerechtigungsWert,
        channel_id: Option<Uuid>,
    ) -> DbResult<()> {
        let id = Uuid::new_v4().to_string();
        let target_type = ziel.typ_str();
        let target_id = ziel.id().map(|u| u.to_string());
        let ch_id = channel_id.map(|u| u.to_string());

        let (value_type, tri_state, int_limit, scope_json) = wert_zu_spalten(&wert)?;

        // Manueller Upsert via DELETE + INSERT (ON CONFLICT funktioniert nicht mit NULL-Spalten)
        let mut tx = self.pool.begin().await?;

        // Bestehenden Eintrag loeschen (wenn vorhanden)
        sqlx::query(
            "DELETE FROM permissions
             WHERE target_type = ?
               AND (target_id = ? OR (target_id IS NULL AND ? IS NULL))
               AND permission_key = ?
               AND (channel_id = ? OR (channel_id IS NULL AND ? IS NULL))",
        )
        .bind(target_type)
        .bind(&target_id)
        .bind(&target_id)
        .bind(permission_key)
        .bind(&ch_id)
        .bind(&ch_id)
        .execute(&mut *tx)
        .await?;

        // Neuen Eintrag einfuegen
        sqlx::query(
            "INSERT INTO permissions
               (id, target_type, target_id, permission_key, value_type, tri_state, int_limit, scope_json, channel_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(target_type)
        .bind(&target_id)
        .bind(permission_key)
        .bind(value_type)
        .bind(tri_state)
        .bind(int_limit)
        .bind(scope_json)
        .bind(&ch_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    async fn remove_permission(
        &self,
        ziel: &BerechtigungsZiel,
        permission_key: &str,
        channel_id: Option<Uuid>,
    ) -> DbResult<bool> {
        let target_type = ziel.typ_str();
        let target_id = ziel.id().map(|u| u.to_string());
        let ch_id = channel_id.map(|u| u.to_string());

        let affected = sqlx::query(
            "DELETE FROM permissions
             WHERE target_type = ?
               AND (target_id = ? OR (target_id IS NULL AND ? IS NULL))
               AND permission_key = ?
               AND (channel_id = ? OR (channel_id IS NULL AND ? IS NULL))",
        )
        .bind(target_type)
        .bind(&target_id)
        .bind(&target_id)
        .bind(permission_key)
        .bind(&ch_id)
        .bind(&ch_id)
        .execute(&self.pool)
        .await?
        .rows_affected();

        Ok(affected > 0)
    }

    async fn resolve_effective_permissions(
        &self,
        user_id: Uuid,
        channel_id: Uuid,
    ) -> DbResult<Vec<EffektiveBerechtigung>> {
        // 1. Individuelle Berechtigungen des Users
        let individual = self
            .get_permissions(&BerechtigungsZiel::Benutzer(user_id), Some(channel_id))
            .await?;

        // 2. Kanal-Gruppe des Users in diesem Kanal
        let kanal_gruppe_perms = {
            let row = sqlx::query(
                "SELECT cg.id FROM channel_groups cg
                 JOIN user_channel_groups ucg ON ucg.group_id = cg.id
                 WHERE ucg.user_id = ? AND ucg.channel_id = ?",
            )
            .bind(user_id.to_string())
            .bind(channel_id.to_string())
            .fetch_optional(&self.pool)
            .await?;

            if let Some(r) = row {
                let kg_id_str: String = r.try_get("id")?;
                let kg_id = Uuid::parse_str(&kg_id_str)
                    .map_err(|e| DbError::intern(format!("Ungueltige KanalGruppe UUID: {e}")))?;
                Some(
                    self.get_permissions(&BerechtigungsZiel::KanalGruppe(kg_id), Some(channel_id))
                        .await?,
                )
            } else {
                None
            }
        };

        // 3. Kanal-Default
        let kanal_default = self
            .get_permissions(
                &BerechtigungsZiel::KanalDefault(channel_id),
                Some(channel_id),
            )
            .await?;

        // 4. Server-Gruppen des Users (nach Prioritaet absteigend)
        let server_gruppen = {
            use crate::repository::ServerGroupRepository;
            let gruppen = ServerGroupRepository::list_for_user(self, user_id).await?;
            let mut result = Vec::new();
            for gruppe in gruppen {
                let perms = self
                    .get_permissions(&BerechtigungsZiel::ServerGruppe(gruppe.id), None)
                    .await?;
                result.push((gruppe.name, perms));
            }
            result
        };

        // 5. Server-Default
        let server_default = self
            .get_permissions(&BerechtigungsZiel::ServerDefault, None)
            .await?;

        let eingabe = BerechtigungsEingabe {
            individual,
            kanal_gruppe: kanal_gruppe_perms,
            kanal_default,
            server_gruppen,
            server_default,
        };

        let aufgeloest = berechtigungen_aufloesen(&eingabe);

        Ok(aufgeloest
            .into_values()
            .map(|a| EffektiveBerechtigung {
                permission_key: a.permission_key,
                wert: a.wert,
                quelle: a.stufe.to_string(),
            })
            .collect())
    }
}

fn row_to_permission(row: &sqlx::sqlite::SqliteRow) -> DbResult<(String, BerechtigungsWert)> {
    let key: String = row.try_get("permission_key")?;
    let value_type: String = row.try_get("value_type")?;

    let wert = match value_type.as_str() {
        "tri_state" => {
            let ts: Option<i64> = row.try_get("tri_state")?;
            BerechtigungsWert::TriState(TriState::from_opt_int(ts))
        }
        "int_limit" => {
            let limit: i64 = row.try_get("int_limit")?;
            BerechtigungsWert::IntLimit(limit)
        }
        "scope" => {
            let json: String = row.try_get("scope_json")?;
            let scope: Vec<String> = serde_json::from_str(&json)
                .map_err(|e| DbError::intern(format!("Ungueltige scope JSON: {e}")))?;
            BerechtigungsWert::Scope(scope)
        }
        other => return Err(DbError::intern(format!("Unbekannter value_type: {other}"))),
    };

    Ok((key, wert))
}

type WertSpalten = (&'static str, Option<i64>, Option<i64>, Option<String>);

fn wert_zu_spalten(wert: &BerechtigungsWert) -> DbResult<WertSpalten> {
    match wert {
        BerechtigungsWert::TriState(ts) => Ok(("tri_state", ts.to_opt_int(), None, None)),
        BerechtigungsWert::IntLimit(limit) => Ok(("int_limit", None, Some(*limit), None)),
        BerechtigungsWert::Scope(scope) => {
            let json = serde_json::to_string(scope)?;
            Ok(("scope", None, None, Some(json)))
        }
    }
}
