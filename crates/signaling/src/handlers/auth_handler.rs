//! Auth-Handler – Login, Logout, Session-Validierung
//!
//! Verarbeitet alle auth-bezogenen ControlMessages und delegiert
//! an den AuthService. Bei Erfolg wird die Session im Verbindungszustand
//! gespeichert.

use crate::error::SignalingResult;
use crate::server_state::SignalingState;
use speakeasy_core::types::{ChannelId, UserId};
use speakeasy_db::{
    models::BenutzerUpdate,
    repository::UserRepository,
    BanRepository, ChannelRepository, ChatMessageRepository, PermissionRepository,
    ServerGroupRepository,
};
use speakeasy_protocol::control::{
    ControlMessage, ControlPayload, ErrorCode, LoginRequest, LoginResponse, LogoutResponse,
    NicknameChangeRequest, NicknameChangeResponse, PasswordChangeRequest, PasswordChangeResponse,
    SetAwayRequest, SetAwayResponse,
};
use std::sync::Arc;

/// Verarbeitet eine Login-Anfrage
///
/// Prueft Credentials, erstellt eine Session und gibt LoginResponse zurueck.
/// Ban-Pruefung findet VOR dem eigentlichen Login statt.
pub async fn handle_login<U, P, B>(
    request: LoginRequest,
    request_id: u32,
    peer_ip: &str,
    state: &Arc<SignalingState<U, P, B>>,
) -> ControlMessage
where
    U: UserRepository + ServerGroupRepository + ChannelRepository + ChatMessageRepository + 'static,
    P: PermissionRepository + 'static,
    B: BanRepository + 'static,
{
    // Ban-Pruefung nach IP
    match state.ban_service.ban_pruefen(None, Some(peer_ip)).await {
        Err(speakeasy_auth::AuthError::IpGebannt(ip)) => {
            tracing::warn!(ip = %ip, "Login von gebannter IP abgelehnt");
            return ControlMessage::error(
                request_id,
                ErrorCode::Banned,
                format!("IP gebannt: {}", ip),
            );
        }
        Err(e) => {
            tracing::error!("Ban-Pruefung fehlgeschlagen: {}", e);
            return ControlMessage::error(request_id, ErrorCode::InternalError, "Interner Fehler");
        }
        Ok(()) => {}
    }

    // Authentifizierung (Passwort oder API-Token)
    let (benutzer, session) = if let Some(ref token) = request.token {
        // API-Token-Authentifizierung
        match state.auth_service.api_token_validieren(token).await {
            Ok((user, _scopes)) => {
                // Fuer API-Token erstellen wir eine temporaere Session
                match state.auth_service.anmelden(&user.username, "").await {
                    Ok(result) => result,
                    Err(_) => {
                        // Direkte Session-Erstellung via Token-Validierung
                        match state.auth_service.session_validieren(token).await {
                            Ok(_u) => {
                                tracing::warn!("Fallback-Session-Validierung fuer Token");
                                return ControlMessage::error(
                                    request_id,
                                    ErrorCode::InvalidCredentials,
                                    "Token-Authentifizierung nicht unterstuetzt",
                                );
                            }
                            Err(_) => {
                                return ControlMessage::error(
                                    request_id,
                                    ErrorCode::InvalidCredentials,
                                    "Ungueltige Anmeldedaten",
                                );
                            }
                        }
                    }
                }
            }
            Err(_) => {
                return ControlMessage::error(
                    request_id,
                    ErrorCode::InvalidCredentials,
                    "Ungueltige Anmeldedaten",
                );
            }
        }
    } else {
        // Passwort-Authentifizierung
        match state
            .auth_service
            .anmelden(&request.username, &request.password)
            .await
        {
            Ok(result) => result,
            Err(speakeasy_auth::AuthError::UngueltigeAnmeldedaten) => {
                tracing::warn!(username = %request.username, "Fehlgeschlagener Login");
                return ControlMessage::error(
                    request_id,
                    ErrorCode::InvalidCredentials,
                    "Ungueltige Anmeldedaten",
                );
            }
            Err(speakeasy_auth::AuthError::BenutzerGesperrt) => {
                return ControlMessage::error(request_id, ErrorCode::Banned, "Benutzer gesperrt");
            }
            Err(speakeasy_auth::AuthError::BenutzerGebannt(grund)) => {
                return ControlMessage::error(
                    request_id,
                    ErrorCode::Banned,
                    format!("Gebannt: {}", grund),
                );
            }
            Err(e) => {
                tracing::error!("Login-Fehler: {}", e);
                return ControlMessage::error(
                    request_id,
                    ErrorCode::InternalError,
                    "Interner Fehler",
                );
            }
        }
    };

    // Ban-Pruefung nach User-ID
    match state.ban_service.ban_pruefen(Some(benutzer.id), None).await {
        Err(speakeasy_auth::AuthError::BenutzerGebannt(grund)) => {
            // Session sofort wieder invalidieren
            let _ = state.auth_service.abmelden(&session.token).await;
            return ControlMessage::error(
                request_id,
                ErrorCode::Banned,
                format!("Gebannt: {}", grund),
            );
        }
        Err(e) => {
            tracing::error!("Ban-Pruefung fehlgeschlagen: {}", e);
        }
        Ok(()) => {}
    }

    // Ablaufzeit berechnen (chrono DateTime -> Unix-Timestamp)
    let expires_at = session.laeuft_ab_am.timestamp() as u64;

    // Server-Gruppen des Benutzers aus der Datenbank laden
    let server_groups = match state.db.list_for_user(benutzer.id).await {
        Ok(gruppen) => gruppen.into_iter().map(|g| g.name).collect(),
        Err(e) => {
            tracing::warn!(
                user_id = %benutzer.id,
                fehler = %e,
                "Server-Gruppen konnten nicht geladen werden"
            );
            vec![]
        }
    };

    let user_id = UserId(benutzer.id);

    // Client in Presence registrieren
    state.presence.client_verbunden(crate::presence::ClientPresence {
        user_id,
        username: benutzer.username.clone(),
        display_name: benutzer.username.clone(),
        channel_id: None,
        is_input_muted: false,
        is_output_muted: false,
        is_away: false,
        away_message: None,
    });

    // Auto-Join: User automatisch in Default-Channel bewegen
    if let Ok(Some(default_channel)) = ChannelRepository::get_default(state.db.as_ref()).await {
        let channel_id = ChannelId(default_channel.id);
        state.presence.channel_beitreten(user_id, channel_id);
        state.broadcaster.channel_beitreten(user_id, channel_id);
        tracing::debug!(
            user_id = %user_id,
            channel_id = %channel_id,
            channel_name = %default_channel.name,
            "User automatisch in Default-Channel eingetreten"
        );
    }

    tracing::info!(
        user_id = %benutzer.id,
        username = %benutzer.username,
        gruppen = ?server_groups,
        "Login erfolgreich"
    );

    let must_change_password = !benutzer.password_changed;

    ControlMessage::new(
        request_id,
        ControlPayload::LoginResponse(LoginResponse {
            user_id,
            session_token: session.token,
            server_id: state.config.server_id,
            expires_at,
            server_groups,
            must_change_password,
        }),
    )
}

/// Verarbeitet eine Logout-Anfrage
pub async fn handle_logout<U, P, B>(
    session_token: &str,
    request_id: u32,
    state: &Arc<SignalingState<U, P, B>>,
) -> ControlMessage
where
    U: UserRepository + ServerGroupRepository + ChannelRepository + ChatMessageRepository + 'static,
    P: PermissionRepository + 'static,
    B: BanRepository + 'static,
{
    match state.auth_service.abmelden(session_token).await {
        Ok(()) => {
            tracing::debug!("Logout erfolgreich");
            ControlMessage::new(
                request_id,
                ControlPayload::LogoutResponse(LogoutResponse { success: true }),
            )
        }
        Err(e) => {
            tracing::warn!("Logout-Fehler: {}", e);
            // Logout gilt auch bei Fehler als "erfolgreich" (idempotent)
            ControlMessage::new(
                request_id,
                ControlPayload::LogoutResponse(LogoutResponse { success: true }),
            )
        }
    }
}

/// Verarbeitet eine Passwort-Aenderungs-Anfrage
pub async fn handle_password_change<U, P, B>(
    request: PasswordChangeRequest,
    request_id: u32,
    user_id: UserId,
    state: &Arc<SignalingState<U, P, B>>,
) -> ControlMessage
where
    U: UserRepository + ServerGroupRepository + ChannelRepository + ChatMessageRepository + 'static,
    P: PermissionRepository + 'static,
    B: BanRepository + 'static,
{
    match state
        .auth_service
        .passwort_aendern(user_id.inner(), &request.old_password, &request.new_password)
        .await
    {
        Ok(()) => {
            // password_changed Flag in DB setzen
            let _ = UserRepository::update(
                state.db.as_ref(),
                user_id.inner(),
                BenutzerUpdate {
                    password_changed: Some(true),
                    ..Default::default()
                },
            )
            .await;
            tracing::info!(user_id = %user_id, "Passwort erfolgreich geaendert");
            ControlMessage::new(
                request_id,
                ControlPayload::PasswordChangeResponse(PasswordChangeResponse { success: true }),
            )
        }
        Err(speakeasy_auth::AuthError::UngueltigeAnmeldedaten) => {
            tracing::warn!(user_id = %user_id, "Passwort-Aenderung: falsches altes Passwort");
            ControlMessage::error(
                request_id,
                ErrorCode::InvalidCredentials,
                "Altes Passwort ist falsch",
            )
        }
        Err(e) => {
            tracing::error!(user_id = %user_id, fehler = %e, "Passwort-Aenderung fehlgeschlagen");
            ControlMessage::error(request_id, ErrorCode::InternalError, "Interner Fehler")
        }
    }
}

/// Verarbeitet eine Nickname-Aenderungs-Anfrage
pub async fn handle_nickname_change<U, P, B>(
    request: NicknameChangeRequest,
    request_id: u32,
    user_id: UserId,
    state: &Arc<SignalingState<U, P, B>>,
) -> ControlMessage
where
    U: UserRepository + ServerGroupRepository + ChannelRepository + ChatMessageRepository + 'static,
    P: PermissionRepository + 'static,
    B: BanRepository + 'static,
{
    let nickname = request.new_nickname.trim().to_string();

    if nickname.is_empty() || nickname.len() > 64 {
        return ControlMessage::error(
            request_id,
            ErrorCode::InvalidRequest,
            "Nickname muss 1-64 Zeichen lang sein",
        );
    }

    // Username in der DB aktualisieren
    match UserRepository::update(
        state.db.as_ref(),
        user_id.inner(),
        BenutzerUpdate {
            username: Some(nickname.clone()),
            ..Default::default()
        },
    )
    .await
    {
        Ok(_) => {
            // Presence-Anzeigename aktualisieren und Broadcast
            state.presence.nickname_aktualisieren(user_id, nickname.clone());

            tracing::info!(
                user_id = %user_id,
                nickname = %nickname,
                "Nickname erfolgreich geaendert"
            );
            ControlMessage::new(
                request_id,
                ControlPayload::NicknameChangeResponse(NicknameChangeResponse { nickname }),
            )
        }
        Err(e) => {
            tracing::error!(user_id = %user_id, fehler = %e, "Nickname-Aenderung fehlgeschlagen");
            ControlMessage::error(request_id, ErrorCode::InternalError, "Interner Fehler")
        }
    }
}

/// Verarbeitet eine Away-Status-Aenderung
pub async fn handle_set_away<U, P, B>(
    request: SetAwayRequest,
    request_id: u32,
    user_id: UserId,
    state: &Arc<SignalingState<U, P, B>>,
) -> ControlMessage
where
    U: UserRepository + ServerGroupRepository + ChannelRepository + ChatMessageRepository + 'static,
    P: PermissionRepository + 'static,
    B: BanRepository + 'static,
{
    state
        .presence
        .away_setzen(user_id, request.away, request.message);

    tracing::debug!(
        user_id = %user_id,
        away = request.away,
        "Away-Status aktualisiert"
    );

    ControlMessage::new(
        request_id,
        ControlPayload::SetAwayResponse(SetAwayResponse { away: request.away }),
    )
}

/// Validiert einen Session-Token und gibt die UserId zurueck
pub async fn session_validieren<U, P, B>(
    token: &str,
    state: &Arc<SignalingState<U, P, B>>,
) -> SignalingResult<UserId>
where
    U: UserRepository + ServerGroupRepository + ChannelRepository + ChatMessageRepository + 'static,
    P: PermissionRepository + 'static,
    B: BanRepository + 'static,
{
    let benutzer = state.auth_service.session_validieren(token).await?;
    Ok(UserId(benutzer.id))
}
