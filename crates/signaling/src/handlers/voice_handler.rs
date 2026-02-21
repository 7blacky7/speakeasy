//! Voice-Handler – VoiceInit, VoiceReady, VoiceDisconnect
//!
//! UDP Port Negotiation und SSRC-Zuweisung fuer Voice-Verbindungen.
//! Koordiniert den Handshake zwischen TCP-Kontrollebene und UDP-Voice-Layer.

use speakeasy_core::types::UserId;
use speakeasy_db::{
    repository::UserRepository, BanRepository, ChannelRepository, ChatMessageRepository,
    PermissionRepository, ServerGroupRepository,
};
use speakeasy_protocol::control::{
    ControlMessage, ControlPayload, VoiceDisconnectRequest, VoiceInitRequest, VoiceReadyResponse,
};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use crate::server_state::SignalingState;

/// Globaler SSRC-Zaehler (atomar, thread-safe)
///
/// Beginnt bei 1 (0 ist reserviert) und inkrementiert monoton.
static SSRC_ZAEHLER: AtomicU32 = AtomicU32::new(1);

/// Weist die naechste verfuegbare SSRC zu
fn naechste_ssrc() -> u32 {
    SSRC_ZAEHLER.fetch_add(1, Ordering::Relaxed)
}

/// Verarbeitet VoiceInit-Anfrage (UDP Port Negotiation)
///
/// Der Client teilt seinen UDP-Port und bevorzugten Codec mit.
/// Der Server antwortet mit seiner UDP-Adresse und einer SSRC.
pub async fn handle_voice_init<U, P, B>(
    request: VoiceInitRequest,
    request_id: u32,
    user_id: UserId,
    peer_addr: SocketAddr,
    state: &Arc<SignalingState<U, P, B>>,
) -> ControlMessage
where
    U: UserRepository + ServerGroupRepository + ChannelRepository + ChatMessageRepository + 'static,
    P: PermissionRepository + 'static,
    B: BanRepository + 'static,
{
    // Codec aushandeln (aktuell nur Opus unterstuetzt)
    let akzeptierter_codec = if request.preferred_codec.to_lowercase() == "opus" {
        "opus".to_string()
    } else {
        tracing::warn!(
            user_id = %user_id,
            codec = %request.preferred_codec,
            "Unbekannter Codec angefordert, fallback auf opus"
        );
        "opus".to_string()
    };

    // UDP-Endpunkt des Clients aus der TCP-Verbindung + Client-Port ableiten
    let client_udp_addr = SocketAddr::new(peer_addr.ip(), request.client_udp_port);

    // SSRC zuweisen
    let ssrc = naechste_ssrc();

    // Client im VoiceState registrieren (ohne Channel – Channel-Zuweisung erfolgt bei Join)
    state
        .voice_state
        .client_registrieren(user_id, ssrc, client_udp_addr);

    tracing::info!(
        user_id = %user_id,
        ssrc,
        client_udp = %client_udp_addr,
        codec = %akzeptierter_codec,
        "Voice-Init erfolgreich"
    );

    // Krypto-Modus und DTLS-Fingerprint aus Server-Konfiguration laden
    let crypto_mode = state.config.crypto_mode.clone();
    let server_dtls_fingerprint = state.config.dtls_fingerprint.clone();

    if crypto_mode != "none" && server_dtls_fingerprint.is_none() {
        tracing::warn!(
            user_id = %user_id,
            crypto_mode = %crypto_mode,
            "Krypto-Modus konfiguriert aber kein DTLS-Fingerprint verfuegbar"
        );
    }

    ControlMessage::new(
        request_id,
        ControlPayload::VoiceReady(VoiceReadyResponse {
            server_udp_port: state.config.voice_udp_port,
            server_ip: state.config.voice_server_ip.clone(),
            ssrc,
            codec: akzeptierter_codec,
            server_dtls_fingerprint,
            crypto_mode,
        }),
    )
}

/// Verarbeitet VoiceDisconnect-Anfrage
///
/// Entfernt den Client aus dem Voice-State und dem Channel-Router.
pub async fn handle_voice_disconnect<U, P, B>(
    request: VoiceDisconnectRequest,
    request_id: u32,
    user_id: UserId,
    state: &Arc<SignalingState<U, P, B>>,
) -> ControlMessage
where
    U: UserRepository + ServerGroupRepository + ChannelRepository + ChatMessageRepository + 'static,
    P: PermissionRepository + 'static,
    B: BanRepository + 'static,
{
    let grund = request.reason.as_deref().unwrap_or("Kein Grund");

    // Aus Voice-State entfernen
    if state.voice_state.client_entfernen(&user_id).is_some() {
        tracing::info!(
            user_id = %user_id,
            grund = %grund,
            "Voice-Verbindung getrennt"
        );
    } else {
        tracing::debug!(user_id = %user_id, "Voice-Disconnect fuer nicht-registrierten Client");
    }

    // Aus Channel-Router entfernen
    state.channel_router.kanal_verlassen(&user_id);

    // Bestaetigung mit leerer Pong-Nachricht
    ControlMessage::new(
        request_id,
        ControlPayload::VoiceReady(VoiceReadyResponse {
            server_udp_port: 0,
            server_ip: String::new(),
            ssrc: 0,
            codec: String::new(),
            server_dtls_fingerprint: None,
            crypto_mode: "none".to_string(),
        }),
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ssrc_monoton_steigend() {
        let a = naechste_ssrc();
        let b = naechste_ssrc();
        let c = naechste_ssrc();
        assert!(b > a);
        assert!(c > b);
    }

    #[test]
    fn ssrc_nie_null() {
        // SSRCs beginnen bei 1, nie bei 0
        for _ in 0..10 {
            assert_ne!(naechste_ssrc(), 0);
        }
    }
}
