//! Chat-Handler – Nachrichten senden, editieren, loeschen, History
//!
//! Routet Chat-Nachrichten ueber den ChatService und sendet
//! eingehende Nachrichten an alle Clients im Channel.

use speakeasy_core::types::UserId;
use speakeasy_db::{
    repository::UserRepository, BanRepository, ChannelRepository, ChatMessageRepository,
    PermissionRepository, ServerGroupRepository,
};
use speakeasy_protocol::control::{
    ChatDeleteRequest, ChatEditRequest, ChatHistoryRequest, ChatHistoryResponse, ChatMessageInfo,
    ChatSendRequest, ChatSendResponse, ControlMessage, ControlPayload, ErrorCode,
};
use std::sync::Arc;

use crate::server_state::SignalingState;

/// Verarbeitet eine Chat-Nachricht
pub async fn handle_chat_send<U, P, B>(
    request: ChatSendRequest,
    request_id: u32,
    user_id: UserId,
    state: &Arc<SignalingState<U, P, B>>,
) -> ControlMessage
where
    U: UserRepository + ServerGroupRepository + ChannelRepository + ChatMessageRepository + 'static,
    P: PermissionRepository + 'static,
    B: BanRepository + 'static,
{
    let reply_to = request
        .reply_to
        .as_deref()
        .and_then(|s| uuid::Uuid::parse_str(s).ok());

    match state
        .chat_service
        .nachricht_senden(
            request.channel_id.inner(),
            user_id.inner(),
            &request.content,
            reply_to,
        )
        .await
    {
        Ok(nachricht) => {
            let created_at = nachricht.created_at.timestamp() as u64;

            // Nachricht an alle Clients im Channel weiterleiten
            let broadcast_msg = ControlMessage::new(
                0,
                ControlPayload::ChatSendResponse(ChatSendResponse {
                    message_id: nachricht.id.to_string(),
                    channel_id: request.channel_id,
                    created_at,
                }),
            );
            state.broadcaster.an_channel_ausser_senden(
                &request.channel_id,
                &user_id,
                broadcast_msg,
            );

            tracing::debug!(
                user_id = %user_id,
                channel_id = %request.channel_id,
                message_id = %nachricht.id,
                "Chat-Nachricht gesendet"
            );

            ControlMessage::new(
                request_id,
                ControlPayload::ChatSendResponse(ChatSendResponse {
                    message_id: nachricht.id.to_string(),
                    channel_id: request.channel_id,
                    created_at,
                }),
            )
        }
        Err(e) => {
            tracing::warn!(
                user_id = %user_id,
                fehler = %e,
                "Chat-Nachricht senden fehlgeschlagen"
            );
            ControlMessage::error(
                request_id,
                ErrorCode::InvalidRequest,
                format!("Nachricht konnte nicht gesendet werden: {}", e),
            )
        }
    }
}

/// Verarbeitet Chat-Nachricht editieren
pub async fn handle_chat_edit<U, P, B>(
    request: ChatEditRequest,
    request_id: u32,
    user_id: UserId,
    state: &Arc<SignalingState<U, P, B>>,
) -> ControlMessage
where
    U: UserRepository + ServerGroupRepository + ChannelRepository + ChatMessageRepository + 'static,
    P: PermissionRepository + 'static,
    B: BanRepository + 'static,
{
    let message_id = match uuid::Uuid::parse_str(&request.message_id) {
        Ok(id) => id,
        Err(_) => {
            return ControlMessage::error(
                request_id,
                ErrorCode::InvalidRequest,
                "Ungueltige Nachrichten-ID",
            );
        }
    };

    match state
        .chat_service
        .nachricht_editieren(message_id, user_id.inner(), &request.content)
        .await
    {
        Ok(_) => {
            tracing::debug!(
                user_id = %user_id,
                message_id = %message_id,
                "Chat-Nachricht editiert"
            );
            ControlMessage::new(request_id, ControlPayload::ChatEdit(request))
        }
        Err(e) => ControlMessage::error(
            request_id,
            ErrorCode::PermissionDenied,
            format!("Nachricht konnte nicht editiert werden: {}", e),
        ),
    }
}

/// Verarbeitet Chat-Nachricht loeschen
pub async fn handle_chat_delete<U, P, B>(
    request: ChatDeleteRequest,
    request_id: u32,
    user_id: UserId,
    state: &Arc<SignalingState<U, P, B>>,
) -> ControlMessage
where
    U: UserRepository + ServerGroupRepository + ChannelRepository + ChatMessageRepository + 'static,
    P: PermissionRepository + 'static,
    B: BanRepository + 'static,
{
    let message_id = match uuid::Uuid::parse_str(&request.message_id) {
        Ok(id) => id,
        Err(_) => {
            return ControlMessage::error(
                request_id,
                ErrorCode::InvalidRequest,
                "Ungueltige Nachrichten-ID",
            );
        }
    };

    match state
        .chat_service
        .nachricht_loeschen(message_id, user_id.inner())
        .await
    {
        Ok(()) => {
            tracing::debug!(
                user_id = %user_id,
                message_id = %message_id,
                "Chat-Nachricht geloescht"
            );
            ControlMessage::new(request_id, ControlPayload::ChatDelete(request))
        }
        Err(e) => ControlMessage::error(
            request_id,
            ErrorCode::PermissionDenied,
            format!("Nachricht konnte nicht geloescht werden: {}", e),
        ),
    }
}

/// Verarbeitet Chat-History-Anfrage
pub async fn handle_chat_history<U, P, B>(
    request: ChatHistoryRequest,
    request_id: u32,
    state: &Arc<SignalingState<U, P, B>>,
) -> ControlMessage
where
    U: UserRepository + ServerGroupRepository + ChannelRepository + ChatMessageRepository + 'static,
    P: PermissionRepository + 'static,
    B: BanRepository + 'static,
{
    let before: Option<chrono::DateTime<chrono::Utc>> = request
        .before
        .as_deref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&chrono::Utc));

    let limit = Some(request.limit.unwrap_or(50).min(100));

    let anfrage = speakeasy_chat::HistoryAnfrage {
        channel_id: request.channel_id.inner(),
        before,
        limit,
    };

    match state.chat_service.history_laden(anfrage).await {
        Ok(nachrichten) => {
            let messages = nachrichten
                .into_iter()
                .map(|n| ChatMessageInfo {
                    message_id: n.id.to_string(),
                    channel_id: request.channel_id,
                    sender_id: UserId(n.sender_id),
                    content: n.content,
                    message_type: match n.message_type {
                        speakeasy_chat::NachrichtenTyp::Text => "text".to_string(),
                        speakeasy_chat::NachrichtenTyp::File => "file".to_string(),
                        speakeasy_chat::NachrichtenTyp::System => "system".to_string(),
                    },
                    reply_to: n.reply_to.map(|id| id.to_string()),
                    created_at: n.created_at.to_rfc3339(),
                    edited_at: n.edited_at.map(|dt| dt.to_rfc3339()),
                })
                .collect();

            ControlMessage::new(
                request_id,
                ControlPayload::ChatHistoryResponse(ChatHistoryResponse {
                    channel_id: request.channel_id,
                    messages,
                }),
            )
        }
        Err(e) => {
            tracing::warn!(
                channel_id = %request.channel_id,
                fehler = %e,
                "Chat-History laden fehlgeschlagen"
            );
            ControlMessage::error(
                request_id,
                ErrorCode::InternalError,
                format!("History konnte nicht geladen werden: {}", e),
            )
        }
    }
}
