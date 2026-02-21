//! Client-Handler – List, Kick, Ban, Move, Poke, Update
//!
//! Alle schreibenden Operationen erfordern Berechtigungspruefung.
//! Permission-Keys folgen dem TeamSpeak-aehnlichen Schema (b_client_*).

use speakeasy_core::types::{ChannelId, UserId};
use speakeasy_db::{
    repository::UserRepository, BanRepository, ChannelRepository, ChatMessageRepository,
    PermissionRepository, ServerGroupRepository,
};
use speakeasy_protocol::control::{
    ClientBanRequest, ClientInfo, ClientKickRequest, ClientListResponse, ClientMoveRequest,
    ClientPokeRequest, ClientUpdateRequest, ControlMessage, ControlPayload, ErrorCode,
};
use std::sync::Arc;
use std::time::Duration;

use crate::presence::ClientPresence;
use crate::server_state::SignalingState;

/// Konvertiert ClientPresence in ClientInfo fuer Protokoll-Antworten
fn client_info_aus_presence(presence: &ClientPresence) -> ClientInfo {
    ClientInfo {
        user_id: presence.user_id,
        username: presence.username.clone(),
        display_name: presence.display_name.clone(),
        channel_id: presence.channel_id,
        server_groups: vec![],
        is_muted: presence.is_output_muted,
        is_deafened: presence.is_output_muted,
        is_input_muted: presence.is_input_muted,
    }
}

/// Verarbeitet Client-Listen-Anfrage
pub async fn handle_client_list<U, P, B>(
    request_id: u32,
    state: &Arc<SignalingState<U, P, B>>,
) -> ControlMessage
where
    U: UserRepository + ServerGroupRepository + ChannelRepository + ChatMessageRepository + 'static,
    P: PermissionRepository + 'static,
    B: BanRepository + 'static,
{
    let clients: Vec<ClientInfo> = state
        .presence
        .alle_clients()
        .iter()
        .map(client_info_aus_presence)
        .collect();

    ControlMessage::new(
        request_id,
        ControlPayload::ClientListResponse(ClientListResponse { clients }),
    )
}

/// Verarbeitet Client-Kick
///
/// Erfordert `b_client_kick_server` (Server-Kick) oder
/// `b_client_kick_channel` (Channel-Kick).
pub async fn handle_client_kick<U, P, B>(
    request: ClientKickRequest,
    request_id: u32,
    actor_id: UserId,
    state: &Arc<SignalingState<U, P, B>>,
) -> ControlMessage
where
    U: UserRepository + ServerGroupRepository + ChannelRepository + ChatMessageRepository + 'static,
    P: PermissionRepository + 'static,
    B: BanRepository + 'static,
{
    // Berechtigung pruefen
    let permission_key = if request.from_channel_only {
        "b_client_kick_channel"
    } else {
        "b_client_kick_server"
    };

    // Channel-Kontext fuer Berechtigung ermitteln
    let channel_id = state
        .presence
        .channel_von_client(&actor_id)
        .unwrap_or_else(|| ChannelId(uuid::Uuid::nil()));

    match state
        .permission_service
        .berechtigung_pruefen(actor_id.inner(), channel_id.inner(), permission_key)
        .await
    {
        Ok(false) => {
            return ControlMessage::error(
                request_id,
                ErrorCode::PermissionDenied,
                "Keine Kick-Berechtigung",
            );
        }
        Err(e) => tracing::error!("Berechtigungspruefung fehlgeschlagen: {}", e),
        Ok(true) => {}
    }

    // Ziel-Client pruefen
    if !state.presence.ist_online(&request.target_user_id) {
        return ControlMessage::error(request_id, ErrorCode::NotFound, "Client nicht verbunden");
    }

    let grund = request.reason.as_deref().unwrap_or("Gekickt");

    if request.from_channel_only {
        // Nur aus Channel entfernen
        state.presence.channel_verlassen(&request.target_user_id);
        state.broadcaster.channel_verlassen(&request.target_user_id);
        state
            .channel_router
            .kanal_verlassen(&request.target_user_id);
        tracing::info!(
            actor = %actor_id,
            target = %request.target_user_id,
            grund = %grund,
            "Client aus Channel gekickt"
        );
    } else {
        // Vom Server kicken – Verbindung wird vom Dispatcher getrennt
        // Wir senden zuerst eine Benachrichtigung an den Client
        let kick_msg = ControlMessage::error(
            0,
            ErrorCode::InvalidRequest,
            format!("Du wurdest gekickt: {}", grund),
        );
        state
            .broadcaster
            .an_user_senden(&request.target_user_id, kick_msg);

        // Cleanup im Presence-Manager (Verbindungstrennung folgt)
        state.presence.client_getrennt(&request.target_user_id);
        state.broadcaster.client_entfernen(&request.target_user_id);
        state.voice_state.client_entfernen(&request.target_user_id);
        state
            .channel_router
            .kanal_verlassen(&request.target_user_id);

        tracing::info!(
            actor = %actor_id,
            target = %request.target_user_id,
            grund = %grund,
            "Client vom Server gekickt"
        );
    }

    // Bestaetigung
    ControlMessage::new(request_id, ControlPayload::ClientList)
}

/// Verarbeitet Client-Ban
///
/// Erfordert `b_client_ban_server`-Berechtigung.
pub async fn handle_client_ban<U, P, B>(
    request: ClientBanRequest,
    request_id: u32,
    actor_id: UserId,
    state: &Arc<SignalingState<U, P, B>>,
) -> ControlMessage
where
    U: UserRepository + ServerGroupRepository + ChannelRepository + ChatMessageRepository + 'static,
    P: PermissionRepository + 'static,
    B: BanRepository + 'static,
{
    // Berechtigung pruefen
    let channel_id = state
        .presence
        .channel_von_client(&actor_id)
        .unwrap_or_else(|| ChannelId(uuid::Uuid::nil()));

    match state
        .permission_service
        .berechtigung_pruefen(actor_id.inner(), channel_id.inner(), "b_client_ban_server")
        .await
    {
        Ok(false) => {
            return ControlMessage::error(
                request_id,
                ErrorCode::PermissionDenied,
                "Keine Ban-Berechtigung",
            );
        }
        Err(e) => tracing::error!("Berechtigungspruefung fehlgeschlagen: {}", e),
        Ok(true) => {}
    }

    // Ban in der Datenbank anlegen
    let dauer = request.duration_secs.map(Duration::from_secs);
    let grund = request.reason.as_deref().unwrap_or("Gebannt");

    match state
        .ban_service
        .benutzer_bannen(
            Some(actor_id.inner()),
            request.target_user_id.inner(),
            grund,
            dauer,
        )
        .await
    {
        Ok(_ban) => {
            // Gebannten Client trennen
            let ban_msg = ControlMessage::error(
                0,
                ErrorCode::Banned,
                format!("Du wurdest gebannt: {}", grund),
            );
            state
                .broadcaster
                .an_user_senden(&request.target_user_id, ban_msg);
            state.presence.client_getrennt(&request.target_user_id);
            state.broadcaster.client_entfernen(&request.target_user_id);
            state.voice_state.client_entfernen(&request.target_user_id);
            state
                .channel_router
                .kanal_verlassen(&request.target_user_id);

            tracing::info!(
                actor = %actor_id,
                target = %request.target_user_id,
                grund = %grund,
                "Client gebannt"
            );

            ControlMessage::new(request_id, ControlPayload::ClientList)
        }
        Err(e) => {
            tracing::error!("Ban fehlgeschlagen: {}", e);
            ControlMessage::error(request_id, ErrorCode::InternalError, "Ban fehlgeschlagen")
        }
    }
}

/// Verarbeitet Client-Move (in anderen Channel verschieben)
///
/// Erfordert `b_client_move`-Berechtigung.
pub async fn handle_client_move<U, P, B>(
    request: ClientMoveRequest,
    request_id: u32,
    actor_id: UserId,
    state: &Arc<SignalingState<U, P, B>>,
) -> ControlMessage
where
    U: UserRepository + ServerGroupRepository + ChannelRepository + ChatMessageRepository + 'static,
    P: PermissionRepository + 'static,
    B: BanRepository + 'static,
{
    // Berechtigung pruefen
    let channel_id = state
        .presence
        .channel_von_client(&actor_id)
        .unwrap_or_else(|| ChannelId(uuid::Uuid::nil()));

    match state
        .permission_service
        .berechtigung_pruefen(actor_id.inner(), channel_id.inner(), "b_client_move")
        .await
    {
        Ok(false) => {
            return ControlMessage::error(
                request_id,
                ErrorCode::PermissionDenied,
                "Keine Move-Berechtigung",
            );
        }
        Err(e) => tracing::error!("Berechtigungspruefung fehlgeschlagen: {}", e),
        Ok(true) => {}
    }

    if !state.presence.ist_online(&request.target_user_id) {
        return ControlMessage::error(request_id, ErrorCode::NotFound, "Client nicht verbunden");
    }

    // Client in neuen Channel verschieben
    state
        .presence
        .channel_beitreten(request.target_user_id, request.target_channel_id);
    state
        .broadcaster
        .channel_beitreten(request.target_user_id, request.target_channel_id);

    // Den verschobenen Client informieren
    let move_msg = ControlMessage::new(
        0,
        speakeasy_protocol::control::ControlPayload::ChannelJoinResponse(
            speakeasy_protocol::control::ChannelJoinResponse {
                channel_id: request.target_channel_id,
                clients: state
                    .presence
                    .clients_in_channel(&request.target_channel_id)
                    .iter()
                    .filter(|p| p.user_id != request.target_user_id)
                    .map(client_info_aus_presence)
                    .collect(),
            },
        ),
    );
    state
        .broadcaster
        .an_user_senden(&request.target_user_id, move_msg);

    tracing::info!(
        actor = %actor_id,
        target = %request.target_user_id,
        ziel_channel = %request.target_channel_id,
        "Client verschoben"
    );

    ControlMessage::new(request_id, ControlPayload::ClientList)
}

/// Verarbeitet Client-Poke (Anklopfen)
///
/// Erfordert `b_client_poke`-Berechtigung.
pub async fn handle_client_poke<U, P, B>(
    request: ClientPokeRequest,
    request_id: u32,
    actor_id: UserId,
    state: &Arc<SignalingState<U, P, B>>,
) -> ControlMessage
where
    U: UserRepository + ServerGroupRepository + ChannelRepository + ChatMessageRepository + 'static,
    P: PermissionRepository + 'static,
    B: BanRepository + 'static,
{
    // Berechtigung pruefen
    let channel_id = state
        .presence
        .channel_von_client(&actor_id)
        .unwrap_or_else(|| ChannelId(uuid::Uuid::nil()));

    match state
        .permission_service
        .berechtigung_pruefen(actor_id.inner(), channel_id.inner(), "b_client_poke")
        .await
    {
        Ok(false) => {
            return ControlMessage::error(
                request_id,
                ErrorCode::PermissionDenied,
                "Keine Poke-Berechtigung",
            );
        }
        Err(e) => tracing::error!("Berechtigungspruefung fehlgeschlagen: {}", e),
        Ok(true) => {}
    }

    if !state.presence.ist_online(&request.target_user_id) {
        return ControlMessage::error(request_id, ErrorCode::NotFound, "Client nicht verbunden");
    }

    // Poke-Nachricht an Ziel-Client senden
    // Wir senden es als ClientPoke-Echo (der Empfaenger sieht wer gepoked hat)
    let poke_nachricht = ControlMessage::new(
        0,
        ControlPayload::ClientPoke(ClientPokeRequest {
            target_user_id: request.target_user_id,
            message: format!("[Von {}] {}", actor_id, request.message),
        }),
    );
    state
        .broadcaster
        .an_user_senden(&request.target_user_id, poke_nachricht);

    tracing::debug!(
        actor = %actor_id,
        target = %request.target_user_id,
        "Poke gesendet"
    );

    ControlMessage::new(request_id, ControlPayload::ClientList)
}

/// Verarbeitet Client-Update (eigene Infos aktualisieren)
pub async fn handle_client_update<U, P, B>(
    request: ClientUpdateRequest,
    request_id: u32,
    user_id: UserId,
    state: &Arc<SignalingState<U, P, B>>,
) -> ControlMessage
where
    U: UserRepository + ServerGroupRepository + ChannelRepository + ChatMessageRepository + 'static,
    P: PermissionRepository + 'static,
    B: BanRepository + 'static,
{
    // Mute/Deaf-Status aktualisieren
    let current = state.presence.client_presence(&user_id);
    if let Some(presence) = current {
        let neues_input_muted = request.is_input_muted.unwrap_or(presence.is_input_muted);
        let neues_output_muted = request.is_output_muted.unwrap_or(presence.is_output_muted);

        state
            .presence
            .status_aktualisieren(user_id, neues_input_muted, neues_output_muted);

        tracing::debug!(
            user_id = %user_id,
            input_muted = neues_input_muted,
            output_muted = neues_output_muted,
            "Client-Status aktualisiert"
        );
    }

    ControlMessage::new(request_id, ControlPayload::ClientList)
}
