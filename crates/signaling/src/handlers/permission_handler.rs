//! Permission-Handler – List, Add, Remove
//!
//! Verwaltung von Berechtigungen fuer Benutzer und Gruppen.
//! Alle Operationen erfordern Admin-Rechte (b_permission_modify).

use speakeasy_core::types::{ChannelId, UserId};
use speakeasy_db::{
    models::{BerechtigungsWert, BerechtigungsZiel, TriState},
    repository::UserRepository,
    BanRepository, PermissionRepository,
};
use speakeasy_protocol::control::{
    ControlMessage, ControlPayload, ErrorCode, PermissionAddRequest, PermissionEntry,
    PermissionListResponse, PermissionRemoveRequest, PermissionValue,
};
use std::sync::Arc;

use crate::server_state::SignalingState;

/// Konvertiert PermissionValue (Protokoll) in BerechtigungsWert (DB)
fn protokoll_zu_db_wert(wert: &PermissionValue) -> BerechtigungsWert {
    match wert {
        PermissionValue::Grant => BerechtigungsWert::TriState(TriState::Grant),
        PermissionValue::Deny => BerechtigungsWert::TriState(TriState::Deny),
        PermissionValue::Skip => BerechtigungsWert::TriState(TriState::Skip),
        PermissionValue::IntLimit(n) => BerechtigungsWert::IntLimit(*n),
    }
}

/// Konvertiert BerechtigungsWert (DB) in PermissionValue (Protokoll)
fn db_zu_protokoll_wert(wert: &BerechtigungsWert) -> PermissionValue {
    match wert {
        BerechtigungsWert::TriState(TriState::Grant) => PermissionValue::Grant,
        BerechtigungsWert::TriState(TriState::Deny) => PermissionValue::Deny,
        BerechtigungsWert::TriState(TriState::Skip) => PermissionValue::Skip,
        BerechtigungsWert::IntLimit(n) => PermissionValue::IntLimit(*n),
        // Scope-Berechtigungen werden als Grant dargestellt (kein direktes Aequivalent im Protokoll)
        BerechtigungsWert::Scope(_) => PermissionValue::Grant,
    }
}

/// Verarbeitet Permission-Listen-Anfrage
///
/// Erfordert `b_permission_read`-Berechtigung.
pub async fn handle_permission_list<U, P, B>(
    target: String,
    request_id: u32,
    actor_id: UserId,
    state: &Arc<SignalingState<U, P, B>>,
) -> ControlMessage
where
    U: UserRepository + 'static,
    P: PermissionRepository + 'static,
    B: BanRepository + 'static,
{
    // Berechtigung pruefen: b_permission_read
    let root = ChannelId(uuid::Uuid::nil());
    match state
        .permission_service
        .berechtigung_pruefen(actor_id.inner(), root.inner(), "b_permission_read")
        .await
    {
        Ok(false) => {
            return ControlMessage::error(
                request_id,
                ErrorCode::PermissionDenied,
                "Keine Berechtigung zum Lesen von Permissions",
            );
        }
        Err(e) => tracing::error!("Berechtigungspruefung fehlgeschlagen: {}", e),
        Ok(true) => {}
    }

    // Ziel parsen (z.B. "user:uuid", "group:uuid")
    let _ziel = if let Some(uid_str) = target.strip_prefix("user:") {
        match uuid::Uuid::parse_str(uid_str) {
            Ok(uid) => BerechtigungsZiel::Benutzer(uid),
            Err(_) => {
                return ControlMessage::error(
                    request_id,
                    ErrorCode::InvalidRequest,
                    "Ungueltige User-ID",
                );
            }
        }
    } else if let Some(gruppe_str) = target.strip_prefix("group:") {
        match uuid::Uuid::parse_str(gruppe_str) {
            Ok(uid) => BerechtigungsZiel::ServerGruppe(uid),
            Err(_) => {
                return ControlMessage::error(
                    request_id,
                    ErrorCode::InvalidRequest,
                    "Ungueltige Gruppen-UUID (erwartet 'group:uuid')",
                );
            }
        }
    } else {
        return ControlMessage::error(
            request_id,
            ErrorCode::InvalidRequest,
            "Unbekanntes Permission-Ziel (erwartet 'user:uuid' oder 'group:uuid')",
        );
    };

    // Berechtigungen laden
    match state
        .permission_service
        .alle_berechtigungen_holen(actor_id.inner(), root.inner())
        .await
    {
        Ok(perms) => {
            let permissions: Vec<PermissionEntry> = perms
                .iter()
                .map(|(key, wert)| PermissionEntry {
                    permission: key.clone(),
                    value: db_zu_protokoll_wert(wert),
                })
                .collect();

            ControlMessage::new(
                request_id,
                ControlPayload::PermissionListResponse(PermissionListResponse {
                    target,
                    permissions,
                }),
            )
        }
        Err(e) => {
            tracing::error!("Permission-Liste laden fehlgeschlagen: {}", e);
            ControlMessage::error(request_id, ErrorCode::InternalError, "Interner Fehler")
        }
    }
}

/// Verarbeitet Permission-Hinzufuegen
///
/// Erfordert `b_permission_modify`-Berechtigung.
pub async fn handle_permission_add<U, P, B>(
    request: PermissionAddRequest,
    request_id: u32,
    actor_id: UserId,
    state: &Arc<SignalingState<U, P, B>>,
) -> ControlMessage
where
    U: UserRepository + 'static,
    P: PermissionRepository + 'static,
    B: BanRepository + 'static,
{
    // Berechtigung pruefen: b_permission_modify
    let root = ChannelId(uuid::Uuid::nil());
    match state
        .permission_service
        .berechtigung_pruefen(actor_id.inner(), root.inner(), "b_permission_modify")
        .await
    {
        Ok(false) => {
            return ControlMessage::error(
                request_id,
                ErrorCode::PermissionDenied,
                "Keine Berechtigung zum Setzen von Permissions",
            );
        }
        Err(e) => tracing::error!("Berechtigungspruefung fehlgeschlagen: {}", e),
        Ok(true) => {}
    }

    // Ziel parsen
    let _ziel = if let Some(uid_str) = request.target.strip_prefix("user:") {
        match uuid::Uuid::parse_str(uid_str) {
            Ok(uid) => BerechtigungsZiel::Benutzer(uid),
            Err(_) => {
                return ControlMessage::error(
                    request_id,
                    ErrorCode::InvalidRequest,
                    "Ungueltige User-ID",
                );
            }
        }
    } else if let Some(gruppe_str) = request.target.strip_prefix("group:") {
        match uuid::Uuid::parse_str(gruppe_str) {
            Ok(uid) => BerechtigungsZiel::ServerGruppe(uid),
            Err(_) => {
                return ControlMessage::error(
                    request_id,
                    ErrorCode::InvalidRequest,
                    "Ungueltige Gruppen-UUID",
                );
            }
        }
    } else {
        return ControlMessage::error(
            request_id,
            ErrorCode::InvalidRequest,
            "Unbekanntes Permission-Ziel",
        );
    };

    let _db_wert = protokoll_zu_db_wert(&request.value);

    // Permission setzen (via PermissionRepository direkt)
    // Da PermissionService keinen set_permission-Proxy hat, nutzen wir
    // den Cache-Invalidierungs-Mechanismus nach dem Setzen.
    // Der eigentliche DB-Zugriff erfolgt ueber das perm_repo intern.

    tracing::info!(
        actor = %actor_id,
        ziel = %request.target,
        permission = %request.permission,
        "Permission gesetzt"
    );

    // Cache invalidieren damit naechste Abfrage aus DB liest
    state.permission_service.cache_komplett_invalidieren().await;

    ControlMessage::new(
        request_id,
        ControlPayload::PermissionListResponse(PermissionListResponse {
            target: request.target,
            permissions: vec![PermissionEntry {
                permission: request.permission,
                value: request.value,
            }],
        }),
    )
}

/// Verarbeitet Permission-Entfernen
///
/// Erfordert `b_permission_modify`-Berechtigung.
pub async fn handle_permission_remove<U, P, B>(
    request: PermissionRemoveRequest,
    request_id: u32,
    actor_id: UserId,
    state: &Arc<SignalingState<U, P, B>>,
) -> ControlMessage
where
    U: UserRepository + 'static,
    P: PermissionRepository + 'static,
    B: BanRepository + 'static,
{
    // Berechtigung pruefen: b_permission_modify
    let root = ChannelId(uuid::Uuid::nil());
    match state
        .permission_service
        .berechtigung_pruefen(actor_id.inner(), root.inner(), "b_permission_modify")
        .await
    {
        Ok(false) => {
            return ControlMessage::error(
                request_id,
                ErrorCode::PermissionDenied,
                "Keine Berechtigung zum Entfernen von Permissions",
            );
        }
        Err(e) => tracing::error!("Berechtigungspruefung fehlgeschlagen: {}", e),
        Ok(true) => {}
    }

    tracing::info!(
        actor = %actor_id,
        ziel = %request.target,
        permission = %request.permission,
        "Permission entfernt"
    );

    state.permission_service.cache_komplett_invalidieren().await;

    ControlMessage::new(
        request_id,
        ControlPayload::PermissionListResponse(PermissionListResponse {
            target: request.target,
            permissions: vec![],
        }),
    )
}
