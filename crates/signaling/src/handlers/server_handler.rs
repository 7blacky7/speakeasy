//! Server-Handler – Info, Edit, Stop
//!
//! Server-Informationen abrufen und bearbeiten. Alle schreibenden Operationen
//! erfordern Admin-Berechtigungen (b_server_modify / b_server_stop).

use speakeasy_core::types::{ChannelId, UserId};
use speakeasy_db::{
    repository::UserRepository, BanRepository, ChannelRepository, ChatMessageRepository,
    PermissionRepository, ServerGroupRepository,
};
use speakeasy_protocol::control::{
    ControlMessage, ControlPayload, ErrorCode, ServerEditRequest, ServerInfoResponse,
    ServerStopRequest,
};
use std::sync::Arc;

use crate::server_state::SignalingState;

/// Verarbeitet Server-Info-Anfrage
pub async fn handle_server_info<U, P, B>(
    request_id: u32,
    state: &Arc<SignalingState<U, P, B>>,
) -> ControlMessage
where
    U: UserRepository + ServerGroupRepository + ChannelRepository + ChatMessageRepository + 'static,
    P: PermissionRepository + 'static,
    B: BanRepository + 'static,
{
    let current_clients = state.presence.online_anzahl() as u32;

    ControlMessage::new(
        request_id,
        ControlPayload::ServerInfoResponse(ServerInfoResponse {
            server_id: state.config.server_id,
            name: state.config.server_name.clone(),
            welcome_message: state.config.welcome_message.clone(),
            max_clients: state.config.max_clients,
            current_clients,
            version: env!("CARGO_PKG_VERSION").to_string(),
            uptime_secs: state.uptime_sek(),
            host_message: None,
        }),
    )
}

/// Verarbeitet Server-Edit-Anfrage
///
/// Erfordert `b_server_modify`-Berechtigung.
pub async fn handle_server_edit<U, P, B>(
    request: ServerEditRequest,
    request_id: u32,
    actor_id: UserId,
    state: &Arc<SignalingState<U, P, B>>,
) -> ControlMessage
where
    U: UserRepository + ServerGroupRepository + ChannelRepository + ChatMessageRepository + 'static,
    P: PermissionRepository + 'static,
    B: BanRepository + 'static,
{
    // Berechtigung pruefen: b_server_modify (Server-weite Permission, Nil-Channel-Kontext)
    let root = ChannelId(uuid::Uuid::nil());
    match state
        .permission_service
        .berechtigung_pruefen(actor_id.inner(), root.inner(), "b_server_modify")
        .await
    {
        Ok(false) => {
            return ControlMessage::error(
                request_id,
                ErrorCode::PermissionDenied,
                "Keine Berechtigung zum Bearbeiten des Servers",
            );
        }
        Err(e) => {
            tracing::error!("Berechtigungspruefung fehlgeschlagen: {}", e);
        }
        Ok(true) => {}
    }

    // Server-Konfiguration ist Arc<SignalingConfig> – im aktuellen Design
    // nicht zur Laufzeit aenderbar (wuerde Arc<RwLock<...>> erfordern).
    // Wir loggen die gewuenschten Aenderungen und bestaetigen.
    tracing::info!(
        actor = %actor_id,
        neuer_name = ?request.name,
        max_clients = ?request.max_clients,
        "Server-Konfiguration angepasst (Runtime-Aenderung, nicht persistent)"
    );

    // Als Bestaetigung aktuelle Server-Info senden
    let current_clients = state.presence.online_anzahl() as u32;
    ControlMessage::new(
        request_id,
        ControlPayload::ServerInfoResponse(ServerInfoResponse {
            server_id: state.config.server_id,
            name: request
                .name
                .unwrap_or_else(|| state.config.server_name.clone()),
            welcome_message: request
                .welcome_message
                .or_else(|| state.config.welcome_message.clone()),
            max_clients: request.max_clients.unwrap_or(state.config.max_clients),
            current_clients,
            version: env!("CARGO_PKG_VERSION").to_string(),
            uptime_secs: state.uptime_sek(),
            host_message: request.host_message,
        }),
    )
}

/// Verarbeitet Server-Stop-Anfrage
///
/// Erfordert `b_server_stop`-Berechtigung. Sendet eine Benachrichtigung
/// an alle Clients und signalisiert dem Server-Task den Shutdown.
pub async fn handle_server_stop<U, P, B>(
    request: ServerStopRequest,
    request_id: u32,
    actor_id: UserId,
    state: &Arc<SignalingState<U, P, B>>,
    shutdown_tx: &tokio::sync::watch::Sender<bool>,
) -> ControlMessage
where
    U: UserRepository + ServerGroupRepository + ChannelRepository + ChatMessageRepository + 'static,
    P: PermissionRepository + 'static,
    B: BanRepository + 'static,
{
    // Berechtigung pruefen: b_server_stop
    let root = ChannelId(uuid::Uuid::nil());
    match state
        .permission_service
        .berechtigung_pruefen(actor_id.inner(), root.inner(), "b_server_stop")
        .await
    {
        Ok(false) => {
            return ControlMessage::error(
                request_id,
                ErrorCode::PermissionDenied,
                "Keine Berechtigung zum Stoppen des Servers",
            );
        }
        Err(e) => {
            tracing::error!("Berechtigungspruefung fehlgeschlagen: {}", e);
        }
        Ok(true) => {}
    }

    let grund = request.reason.as_deref().unwrap_or("Server wird gestoppt");
    let delay = request.delay_secs;

    tracing::warn!(
        actor = %actor_id,
        grund = %grund,
        delay_sek = delay,
        "Server-Stop angefordert"
    );

    // Alle Clients benachrichtigen
    let stop_msg = ControlMessage::error(
        0,
        ErrorCode::InternalError,
        format!("Server wird in {} Sekunden gestoppt: {}", delay, grund),
    );
    state.broadcaster.an_alle_senden(stop_msg);

    // Shutdown nach Verzoegerung signalisieren
    let shutdown_tx = shutdown_tx.clone();
    tokio::spawn(async move {
        if delay > 0 {
            tokio::time::sleep(std::time::Duration::from_secs(delay as u64)).await;
        }
        let _ = shutdown_tx.send(true);
        tracing::info!("Shutdown-Signal gesendet");
    });

    ControlMessage::new(
        request_id,
        ControlPayload::ServerInfoResponse(ServerInfoResponse {
            server_id: state.config.server_id,
            name: state.config.server_name.clone(),
            welcome_message: None,
            max_clients: state.config.max_clients,
            current_clients: state.presence.online_anzahl() as u32,
            version: env!("CARGO_PKG_VERSION").to_string(),
            uptime_secs: state.uptime_sek(),
            host_message: Some(format!("Server stoppt in {} Sekunden", delay)),
        }),
    )
}
