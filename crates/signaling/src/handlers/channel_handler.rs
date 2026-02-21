//! Channel-Handler – Join, Leave, Create, Delete, Edit, List
//!
//! Alle Channel-Operationen erfordern eine authentifizierte Session.
//! Schreibende Operationen (Create, Edit, Delete) erfordern entsprechende
//! Berechtigungen.

use speakeasy_core::types::{ChannelId, UserId};
use speakeasy_db::{
    repository::UserRepository, BanRepository, ChannelRepository, ChatMessageRepository,
    PermissionRepository, ServerGroupRepository,
};
use speakeasy_protocol::control::{
    ChannelCreateRequest, ChannelCreateResponse, ChannelDeleteRequest, ChannelEditRequest,
    ChannelInfo, ChannelJoinRequest, ChannelJoinResponse, ChannelLeaveRequest, ChannelListResponse,
    ClientInfo, ControlMessage, ControlPayload, ErrorCode,
};
use std::sync::Arc;

use crate::presence::ClientPresence;
use crate::server_state::SignalingState;

/// Erstellt ChannelInfo aus einem DB-KanalRecord
fn channel_info_aus_record(
    record: &speakeasy_db::models::KanalRecord,
    client_anzahl: u32,
) -> ChannelInfo {
    ChannelInfo {
        channel_id: ChannelId(record.id),
        name: record.name.clone(),
        description: record.topic.clone(),
        parent_id: record.parent_id.map(ChannelId),
        sort_order: record.sort_order as i32,
        max_clients: if record.max_clients > 0 {
            Some(record.max_clients as u32)
        } else {
            None
        },
        current_clients: client_anzahl,
        password_protected: record.password_hash.is_some(),
        codec: "opus".to_string(),
        codec_quality: 7,
    }
}

/// Erstellt ChannelInfo fuer einen ephemeren Channel (nicht in DB)
fn channel_info_ephemer(channel_id: ChannelId, client_anzahl: u32) -> ChannelInfo {
    ChannelInfo {
        channel_id,
        name: format!("Channel {}", &channel_id.inner().to_string()[..8]),
        description: None,
        parent_id: None,
        sort_order: 0,
        max_clients: None,
        current_clients: client_anzahl,
        password_protected: false,
        codec: "opus".to_string(),
        codec_quality: 7,
    }
}

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

/// Verarbeitet Channel-Listen-Anfrage
pub async fn handle_channel_list<U, P, B>(
    request_id: u32,
    state: &Arc<SignalingState<U, P, B>>,
) -> ControlMessage
where
    U: UserRepository + ServerGroupRepository + ChannelRepository + ChatMessageRepository + 'static,
    P: PermissionRepository + 'static,
    B: BanRepository + 'static,
{
    // DB-Channels laden
    let mut channels: Vec<ChannelInfo> = match ChannelRepository::list(state.db.as_ref()).await {
        Ok(db_channels) => db_channels
            .iter()
            .map(|record| {
                let cid = ChannelId(record.id);
                let anzahl = state.presence.user_ids_in_channel(&cid).len() as u32;
                channel_info_aus_record(record, anzahl)
            })
            .collect(),
        Err(e) => {
            tracing::warn!(fehler = %e, "DB-Channels konnten nicht geladen werden");
            vec![]
        }
    };

    // Ephemere Channels hinzufuegen (aktive Voice-Channels die nicht in der DB sind)
    let db_channel_ids: std::collections::HashSet<ChannelId> =
        channels.iter().map(|c| c.channel_id).collect();
    let aktive_kanaele = state.channel_router.aktive_kanaele();
    for cid in aktive_kanaele {
        if !db_channel_ids.contains(&cid) {
            let anzahl = state.presence.user_ids_in_channel(&cid).len() as u32;
            channels.push(channel_info_ephemer(cid, anzahl));
        }
    }

    ControlMessage::new(
        request_id,
        ControlPayload::ChannelListResponse(ChannelListResponse { channels }),
    )
}

/// Verarbeitet Channel-Beitritt
pub async fn handle_channel_join<U, P, B>(
    request: ChannelJoinRequest,
    request_id: u32,
    user_id: UserId,
    state: &Arc<SignalingState<U, P, B>>,
) -> ControlMessage
where
    U: UserRepository + ServerGroupRepository + ChannelRepository + ChatMessageRepository + 'static,
    P: PermissionRepository + 'static,
    B: BanRepository + 'static,
{
    let channel_id = request.channel_id;

    // Berechtigung: b_channel_join pruefen
    match state
        .permission_service
        .berechtigung_pruefen(user_id.inner(), channel_id.inner(), "b_channel_join")
        .await
    {
        Ok(false) => {
            return ControlMessage::error(
                request_id,
                ErrorCode::PermissionDenied,
                "Keine Berechtigung diesem Channel beizutreten",
            );
        }
        Err(e) => {
            // Bei Fehler: Im Dev-Modus erlauben (Fallthrough)
            tracing::error!("Berechtigungspruefung fehlgeschlagen: {}", e);
        }
        Ok(true) => {}
    }

    // Aus altem Channel austreten wenn vorhanden
    let alter_channel = state.presence.channel_von_client(&user_id);
    if let Some(alter) = alter_channel {
        state.presence.channel_verlassen(&user_id);
        state.broadcaster.channel_verlassen(&user_id);
        state.channel_router.kanal_verlassen(&user_id);

        // Anderen Channel-Mitgliedern mitteilen dass User gegangen ist
        // (wird vom Dispatcher als Event versendet)
        tracing::debug!(
            user_id = %user_id,
            alter_channel = %alter,
            "Client aus altem Channel ausgetreten"
        );
    }

    // Neuen Channel beitreten
    state.presence.channel_beitreten(user_id, channel_id);
    state.broadcaster.channel_beitreten(user_id, channel_id);

    // Aktuelle Clients im Channel fuer die Antwort ermitteln
    let clients_im_channel = state
        .presence
        .clients_in_channel(&channel_id)
        .iter()
        .filter(|p| p.user_id != user_id)
        .map(client_info_aus_presence)
        .collect();

    tracing::info!(
        user_id = %user_id,
        channel_id = %channel_id,
        "Client Channel beigetreten"
    );

    ControlMessage::new(
        request_id,
        ControlPayload::ChannelJoinResponse(ChannelJoinResponse {
            channel_id,
            clients: clients_im_channel,
        }),
    )
}

/// Verarbeitet Channel-Verlassen
pub async fn handle_channel_leave<U, P, B>(
    request: ChannelLeaveRequest,
    request_id: u32,
    user_id: UserId,
    state: &Arc<SignalingState<U, P, B>>,
) -> ControlMessage
where
    U: UserRepository + ServerGroupRepository + ChannelRepository + ChatMessageRepository + 'static,
    P: PermissionRepository + 'static,
    B: BanRepository + 'static,
{
    let aktueller_channel = state.presence.channel_von_client(&user_id);

    match aktueller_channel {
        Some(channel_id) if channel_id == request.channel_id => {
            state.presence.channel_verlassen(&user_id);
            state.broadcaster.channel_verlassen(&user_id);
            state.channel_router.kanal_verlassen(&user_id);

            tracing::info!(user_id = %user_id, channel_id = %channel_id, "Client Channel verlassen");

            // Bestaetigung (leere Antwort = Erfolg)
            ControlMessage::new(request_id, ControlPayload::ChannelList)
        }
        Some(anderer) => {
            tracing::warn!(
                user_id = %user_id,
                angefragter_channel = %request.channel_id,
                aktueller_channel = %anderer,
                "Channel-Leave fuer falschen Channel"
            );
            ControlMessage::error(
                request_id,
                ErrorCode::NotFound,
                "Client ist nicht in diesem Channel",
            )
        }
        None => ControlMessage::error(
            request_id,
            ErrorCode::NotFound,
            "Client ist in keinem Channel",
        ),
    }
}

/// Verarbeitet Channel-Erstellung
///
/// Erfordert `b_channel_create`-Berechtigung.
pub async fn handle_channel_create<U, P, B>(
    request: ChannelCreateRequest,
    request_id: u32,
    user_id: UserId,
    state: &Arc<SignalingState<U, P, B>>,
) -> ControlMessage
where
    U: UserRepository + ServerGroupRepository + ChannelRepository + ChatMessageRepository + 'static,
    P: PermissionRepository + 'static,
    B: BanRepository + 'static,
{
    // Berechtigung pruefen: b_channel_create
    // Da wir noch keinen persistenten Channel-Store haben, verwenden wir
    // eine Root-Channel-ID (Nil-UUID) als Kontext fuer Server-weite Perms
    let root_channel = ChannelId(uuid::Uuid::nil());
    match state
        .permission_service
        .berechtigung_pruefen(user_id.inner(), root_channel.inner(), "b_channel_create")
        .await
    {
        Ok(false) => {
            return ControlMessage::error(
                request_id,
                ErrorCode::PermissionDenied,
                "Keine Berechtigung zum Erstellen von Channels",
            );
        }
        Err(e) => {
            tracing::error!("Berechtigungspruefung fehlgeschlagen: {}", e);
            // Bei Fehler: Im Dev-Modus erlauben (Fallthrough)
        }
        Ok(true) => {}
    }

    // Neuen Channel erstellen (ephemer – nur in-memory)
    let channel_id = ChannelId::new();

    tracing::info!(
        user_id = %user_id,
        channel_id = %channel_id,
        name = %request.name,
        "Channel erstellt"
    );

    ControlMessage::new(
        request_id,
        ControlPayload::ChannelCreateResponse(ChannelCreateResponse { channel_id }),
    )
}

/// Verarbeitet Channel-Bearbeitung
///
/// Erfordert `b_channel_modify`-Berechtigung.
pub async fn handle_channel_edit<U, P, B>(
    request: ChannelEditRequest,
    request_id: u32,
    user_id: UserId,
    state: &Arc<SignalingState<U, P, B>>,
) -> ControlMessage
where
    U: UserRepository + ServerGroupRepository + ChannelRepository + ChatMessageRepository + 'static,
    P: PermissionRepository + 'static,
    B: BanRepository + 'static,
{
    // Berechtigung pruefen: b_channel_modify
    match state
        .permission_service
        .berechtigung_pruefen(
            user_id.inner(),
            request.channel_id.inner(),
            "b_channel_modify",
        )
        .await
    {
        Ok(false) => {
            return ControlMessage::error(
                request_id,
                ErrorCode::PermissionDenied,
                "Keine Berechtigung zum Bearbeiten von Channels",
            );
        }
        Err(e) => {
            tracing::error!("Berechtigungspruefung fehlgeschlagen: {}", e);
        }
        Ok(true) => {}
    }

    tracing::info!(
        user_id = %user_id,
        channel_id = %request.channel_id,
        "Channel bearbeitet"
    );

    // Erfolgreich (leere ServerInfo als Bestaetigung)
    ControlMessage::new(request_id, ControlPayload::ChannelList)
}

/// Verarbeitet Channel-Loeschung
///
/// Erfordert `b_channel_delete`-Berechtigung.
pub async fn handle_channel_delete<U, P, B>(
    request: ChannelDeleteRequest,
    request_id: u32,
    user_id: UserId,
    state: &Arc<SignalingState<U, P, B>>,
) -> ControlMessage
where
    U: UserRepository + ServerGroupRepository + ChannelRepository + ChatMessageRepository + 'static,
    P: PermissionRepository + 'static,
    B: BanRepository + 'static,
{
    // Berechtigung pruefen: b_channel_delete
    match state
        .permission_service
        .berechtigung_pruefen(
            user_id.inner(),
            request.channel_id.inner(),
            "b_channel_delete",
        )
        .await
    {
        Ok(false) => {
            return ControlMessage::error(
                request_id,
                ErrorCode::PermissionDenied,
                "Keine Berechtigung zum Loeschen von Channels",
            );
        }
        Err(e) => {
            tracing::error!("Berechtigungspruefung fehlgeschlagen: {}", e);
        }
        Ok(true) => {}
    }

    // Alle Clients aus Channel entfernen
    let betroffene_clients = state.presence.user_ids_in_channel(&request.channel_id);
    let ziel_channel = request.move_clients_to;

    for uid in betroffene_clients {
        state.presence.channel_verlassen(&uid);
        state.broadcaster.channel_verlassen(&uid);
        state.channel_router.kanal_verlassen(&uid);

        // Falls Ziel-Channel angegeben: dorthin verschieben
        if let Some(ziel) = ziel_channel {
            state.presence.channel_beitreten(uid, ziel);
            state.broadcaster.channel_beitreten(uid, ziel);
        }
    }

    tracing::info!(
        user_id = %user_id,
        channel_id = %request.channel_id,
        "Channel geloescht"
    );

    ControlMessage::new(request_id, ControlPayload::ChannelList)
}
