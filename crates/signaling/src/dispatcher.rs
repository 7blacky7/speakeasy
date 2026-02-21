//! Message-Dispatcher – Routet ControlMessages an die richtigen Handler
//!
//! Der Dispatcher empfaengt ControlMessages von einer ClientConnection,
//! bestimmt den richtigen Handler und gibt die Antwort zurueck.
//!
//! ## Zustandspruefung
//! Bestimmte Nachrichten sind nur in bestimmten Verbindungszustaenden erlaubt:
//! - `Login` nur im `Connected`/`Authenticating`-Zustand
//! - Alle anderen nur im `Authenticated`/`InChannel`-Zustand

use speakeasy_core::types::UserId;
use speakeasy_db::{
    repository::UserRepository, BanRepository, ChannelRepository, ChatMessageRepository,
    PermissionRepository, ServerGroupRepository,
};
use speakeasy_protocol::control::{ControlMessage, ControlPayload, ErrorCode};
use std::net::SocketAddr;
use std::sync::Arc;

use crate::handlers::{
    auth_handler, channel_handler, chat_handler, client_handler, permission_handler,
    server_handler, voice_handler,
};
use crate::server_state::SignalingState;

/// Dispatcher-Kontext – Informationen ueber die aktuelle Verbindung
pub struct DispatcherContext {
    /// Peer-IP-Adresse fuer Ban-Pruefungen
    pub peer_addr: SocketAddr,
    /// Aktueller Session-Token (None wenn nicht authentifiziert)
    pub session_token: Option<String>,
    /// Authentifizierte User-ID (None wenn nicht authentifiziert)
    pub user_id: Option<UserId>,
    /// Shutdown-Sender fuer Server-Stop-Kommando
    pub shutdown_tx: tokio::sync::watch::Sender<bool>,
}

/// Zentraler Message-Dispatcher
///
/// Routet eingehende ControlMessages an die entsprechenden Handler und
/// gibt die Antwort-ControlMessage zurueck.
pub struct MessageDispatcher<U, P, B>
where
    U: UserRepository + ServerGroupRepository + ChannelRepository + ChatMessageRepository + 'static,
    P: PermissionRepository + 'static,
    B: BanRepository + 'static,
{
    state: Arc<SignalingState<U, P, B>>,
}

impl<U, P, B> MessageDispatcher<U, P, B>
where
    U: UserRepository + ServerGroupRepository + ChannelRepository + ChatMessageRepository + 'static,
    P: PermissionRepository + 'static,
    B: BanRepository + 'static,
{
    /// Erstellt einen neuen Dispatcher
    pub fn neu(state: Arc<SignalingState<U, P, B>>) -> Self {
        Self { state }
    }

    /// Verarbeitet eine eingehende ControlMessage und gibt die Antwort zurueck
    ///
    /// Gibt `None` zurueck wenn keine Antwort gesendet werden soll
    /// (z.B. bei Pong-Antworten die intern verarbeitet werden).
    pub async fn dispatch(
        &self,
        message: ControlMessage,
        ctx: &mut DispatcherContext,
    ) -> Option<ControlMessage> {
        let request_id = message.request_id;

        match message.payload {
            // -------------------------------------------------------------------
            // Auth-Nachrichten (immer erlaubt)
            // -------------------------------------------------------------------
            ControlPayload::Login(req) => {
                // Login nur wenn noch nicht authentifiziert
                if ctx.user_id.is_some() {
                    return Some(ControlMessage::error(
                        request_id,
                        ErrorCode::AlreadyLoggedIn,
                        "Bereits angemeldet",
                    ));
                }

                let peer_ip = ctx.peer_addr.ip().to_string();
                let antwort =
                    auth_handler::handle_login(req, request_id, &peer_ip, &self.state).await;

                // Bei Erfolg: Session-Token und User-ID speichern
                if let ControlPayload::LoginResponse(ref resp) = antwort.payload {
                    ctx.session_token = Some(resp.session_token.clone());
                    ctx.user_id = Some(resp.user_id);
                    tracing::debug!(
                        user_id = %resp.user_id,
                        "Verbindung authentifiziert"
                    );
                }

                Some(antwort)
            }

            ControlPayload::Logout(_req) => {
                let token = match &ctx.session_token {
                    Some(t) => t.clone(),
                    None => {
                        return Some(ControlMessage::error(
                            request_id,
                            ErrorCode::SessionExpired,
                            "Nicht angemeldet",
                        ));
                    }
                };

                // Cleanup vor dem Logout
                if let Some(uid) = ctx.user_id {
                    self.client_cleanup(&uid).await;
                }

                let antwort = auth_handler::handle_logout(&token, request_id, &self.state).await;

                ctx.session_token = None;
                ctx.user_id = None;

                Some(antwort)
            }

            // -------------------------------------------------------------------
            // Keepalive
            // -------------------------------------------------------------------
            ControlPayload::Ping(ping) => {
                let server_ts = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                Some(ControlMessage::pong(
                    request_id,
                    ping.timestamp_ms,
                    server_ts,
                ))
            }

            ControlPayload::Pong(_) => {
                // Pong-Antworten vom Client werden nur geloggt (RTT-Messung)
                tracing::trace!("Pong empfangen (RTT-Messung)");
                None
            }

            // -------------------------------------------------------------------
            // Authentifizierung erfordernde Nachrichten
            // -------------------------------------------------------------------
            payload => {
                let user_id = match ctx.user_id {
                    Some(uid) => uid,
                    None => {
                        return Some(ControlMessage::error(
                            request_id,
                            ErrorCode::SessionExpired,
                            "Nicht authentifiziert – bitte zuerst anmelden",
                        ));
                    }
                };

                self.dispatch_authenticated(payload, request_id, user_id, ctx)
                    .await
            }
        }
    }

    /// Routet Nachrichten die eine Authentifizierung erfordern
    async fn dispatch_authenticated(
        &self,
        payload: ControlPayload,
        request_id: u32,
        user_id: UserId,
        ctx: &mut DispatcherContext,
    ) -> Option<ControlMessage> {
        match payload {
            // -------------------------------------------------------------------
            // Channel-Nachrichten
            // -------------------------------------------------------------------
            ControlPayload::ChannelList => {
                Some(channel_handler::handle_channel_list(request_id, &self.state).await)
            }

            ControlPayload::ChannelJoin(req) => Some(
                channel_handler::handle_channel_join(req, request_id, user_id, &self.state).await,
            ),

            ControlPayload::ChannelLeave(req) => Some(
                channel_handler::handle_channel_leave(req, request_id, user_id, &self.state).await,
            ),

            ControlPayload::ChannelCreate(req) => Some(
                channel_handler::handle_channel_create(req, request_id, user_id, &self.state).await,
            ),

            ControlPayload::ChannelEdit(req) => Some(
                channel_handler::handle_channel_edit(req, request_id, user_id, &self.state).await,
            ),

            ControlPayload::ChannelDelete(req) => Some(
                channel_handler::handle_channel_delete(req, request_id, user_id, &self.state).await,
            ),

            // -------------------------------------------------------------------
            // Client-Nachrichten
            // -------------------------------------------------------------------
            ControlPayload::ClientList => {
                Some(client_handler::handle_client_list(request_id, &self.state).await)
            }

            ControlPayload::ClientKick(req) => Some(
                client_handler::handle_client_kick(req, request_id, user_id, &self.state).await,
            ),

            ControlPayload::ClientBan(req) => {
                Some(client_handler::handle_client_ban(req, request_id, user_id, &self.state).await)
            }

            ControlPayload::ClientMove(req) => Some(
                client_handler::handle_client_move(req, request_id, user_id, &self.state).await,
            ),

            ControlPayload::ClientPoke(req) => Some(
                client_handler::handle_client_poke(req, request_id, user_id, &self.state).await,
            ),

            ControlPayload::ClientUpdate(req) => Some(
                client_handler::handle_client_update(req, request_id, user_id, &self.state).await,
            ),

            // -------------------------------------------------------------------
            // Server-Nachrichten
            // -------------------------------------------------------------------
            ControlPayload::ServerInfo => {
                Some(server_handler::handle_server_info(request_id, &self.state).await)
            }

            ControlPayload::ServerEdit(req) => Some(
                server_handler::handle_server_edit(req, request_id, user_id, &self.state).await,
            ),

            ControlPayload::ServerStop(req) => Some(
                server_handler::handle_server_stop(
                    req,
                    request_id,
                    user_id,
                    &self.state,
                    &ctx.shutdown_tx,
                )
                .await,
            ),

            // -------------------------------------------------------------------
            // Permission-Nachrichten
            // -------------------------------------------------------------------
            ControlPayload::PermissionList { target } => Some(
                permission_handler::handle_permission_list(
                    target,
                    request_id,
                    user_id,
                    &self.state,
                )
                .await,
            ),

            ControlPayload::PermissionAdd(req) => Some(
                permission_handler::handle_permission_add(req, request_id, user_id, &self.state)
                    .await,
            ),

            ControlPayload::PermissionRemove(req) => Some(
                permission_handler::handle_permission_remove(req, request_id, user_id, &self.state)
                    .await,
            ),

            // -------------------------------------------------------------------
            // Voice-Setup-Nachrichten
            // -------------------------------------------------------------------
            ControlPayload::VoiceInit(req) => Some(
                voice_handler::handle_voice_init(
                    req,
                    request_id,
                    user_id,
                    ctx.peer_addr,
                    &self.state,
                )
                .await,
            ),

            ControlPayload::VoiceDisconnect(req) => Some(
                voice_handler::handle_voice_disconnect(req, request_id, user_id, &self.state).await,
            ),

            // -------------------------------------------------------------------
            // Chat-Nachrichten
            // -------------------------------------------------------------------
            ControlPayload::ChatSend(req) => {
                Some(chat_handler::handle_chat_send(req, request_id, user_id, &self.state).await)
            }

            ControlPayload::ChatEdit(req) => {
                Some(chat_handler::handle_chat_edit(req, request_id, user_id, &self.state).await)
            }

            ControlPayload::ChatDelete(req) => {
                Some(chat_handler::handle_chat_delete(req, request_id, user_id, &self.state).await)
            }

            ControlPayload::ChatHistory(req) => {
                Some(chat_handler::handle_chat_history(req, request_id, &self.state).await)
            }

            // -------------------------------------------------------------------
            // Unbekannte / unerwartete Nachrichten
            // -------------------------------------------------------------------
            ControlPayload::LoginResponse(_)
            | ControlPayload::LogoutResponse(_)
            | ControlPayload::ChannelListResponse(_)
            | ControlPayload::ChannelJoinResponse(_)
            | ControlPayload::ChannelCreateResponse(_)
            | ControlPayload::ClientListResponse(_)
            | ControlPayload::ServerInfoResponse(_)
            | ControlPayload::PermissionListResponse(_)
            | ControlPayload::FileListResponse(_)
            | ControlPayload::FileUploadResponse(_)
            | ControlPayload::ChatSendResponse(_)
            | ControlPayload::ChatHistoryResponse(_)
            | ControlPayload::VoiceReady(_)
            | ControlPayload::Error(_) => {
                tracing::warn!(
                    request_id,
                    "Unerwartete Server->Client Nachricht vom Client empfangen"
                );
                Some(ControlMessage::error(
                    request_id,
                    ErrorCode::InvalidRequest,
                    "Unerwartete Nachricht",
                ))
            }

            // File-Nachrichten (noch nicht implementiert)
            ControlPayload::FileList { .. }
            | ControlPayload::FileUpload(_)
            | ControlPayload::FileDelete(_) => Some(ControlMessage::error(
                request_id,
                ErrorCode::InvalidRequest,
                "File-Service noch nicht implementiert",
            )),

            // Ping/Pong werden oben bereits behandelt
            ControlPayload::Ping(_) | ControlPayload::Pong(_) => None,

            // Login/Logout im authentifizierten Zustand – Fehlermeldung
            ControlPayload::Login(_) => Some(ControlMessage::error(
                request_id,
                ErrorCode::AlreadyLoggedIn,
                "Bereits angemeldet",
            )),
            ControlPayload::Logout(_) => Some(ControlMessage::error(
                request_id,
                ErrorCode::InvalidRequest,
                "Logout muss ueber den normalen Pfad erfolgen",
            )),
        }
    }

    /// Bereinigt alle Ressourcen eines Clients beim Trennen
    pub async fn client_cleanup(&self, user_id: &UserId) {
        self.state.presence.client_getrennt(user_id);
        self.state.broadcaster.client_entfernen(user_id);
        self.state.voice_state.client_entfernen(user_id);
        self.state.channel_router.kanal_verlassen(user_id);

        tracing::debug!(user_id = %user_id, "Client-Ressourcen bereinigt");
    }
}
