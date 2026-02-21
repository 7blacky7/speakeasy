//! Gemeinsamer Server-Zustand fuer den Signaling-Service
//!
//! Haelt alle geteilten Services und Zustands-Manager als Arc-Referenzen,
//! die sicher zwischen tokio-Tasks geteilt werden koennen.

use speakeasy_auth::{AuthService, BanService, PermissionService};
use speakeasy_chat::ChatService;
use speakeasy_core::types::ServerId;
use speakeasy_db::{
    repository::UserRepository, BanRepository, ChannelRepository, ChatMessageRepository,
    PermissionRepository, ServerGroupRepository,
};
use speakeasy_voice::{ChannelRouter, VoiceState};
use std::sync::Arc;
use std::time::Instant;

use crate::broadcast::EventBroadcaster;
use crate::presence::PresenceManager;

/// Konfiguration fuer den Signaling-Service
#[derive(Debug, Clone)]
pub struct SignalingConfig {
    /// Server-ID (unveraenderlich nach dem Start)
    pub server_id: ServerId,
    /// Anzeigename des Servers
    pub server_name: String,
    /// Willkommensnachricht
    pub welcome_message: Option<String>,
    /// Maximale Clients
    pub max_clients: u32,
    /// UDP-Port des Voice-Servers (fuer VoiceInit-Antworten)
    pub voice_udp_port: u16,
    /// Server-IP fuer Voice-Verbindungen
    pub voice_server_ip: String,
    /// Keepalive-Intervall in Sekunden
    pub keepalive_sek: u64,
    /// Timeout fuer inaktive Verbindungen in Sekunden
    pub verbindungs_timeout_sek: u64,
    /// Krypto-Modus fuer Voice ("none", "dtls", "e2e")
    pub crypto_mode: String,
    /// DTLS-Fingerprint des Servers (wenn TLS konfiguriert)
    pub dtls_fingerprint: Option<String>,
}

impl Default for SignalingConfig {
    fn default() -> Self {
        Self {
            server_id: ServerId::new(),
            server_name: "Speakeasy Server".to_string(),
            welcome_message: None,
            max_clients: 512,
            voice_udp_port: 9987,
            voice_server_ip: "0.0.0.0".to_string(),
            keepalive_sek: 30,
            verbindungs_timeout_sek: 90,
            crypto_mode: "none".to_string(),
            dtls_fingerprint: None,
        }
    }
}

/// Gemeinsamer Server-Zustand (thread-safe, Arc-geteilt)
///
/// Alle Services sind als Arc gehalten. Clone gibt eine Referenz auf
/// denselben inneren Zustand.
pub struct SignalingState<U, P, B>
where
    U: UserRepository + ServerGroupRepository + ChannelRepository + ChatMessageRepository + 'static,
    P: PermissionRepository + 'static,
    B: BanRepository + 'static,
{
    /// Server-Konfiguration
    pub config: Arc<SignalingConfig>,
    /// Auth-Service (Login, Logout, Session-Validierung)
    pub auth_service: Arc<AuthService<U>>,
    /// Permission-Service (Berechtigungspruefung)
    pub permission_service: Arc<PermissionService<P>>,
    /// Ban-Service (Benutzer- und IP-Bans)
    pub ban_service: Arc<BanService<B>>,
    /// Datenbank-Zugriff (fuer Server-Gruppen, Channels, Chat)
    pub db: Arc<U>,
    /// Chat-Service (Nachrichten senden, History, etc.)
    pub chat_service: Arc<ChatService<U>>,
    /// Voice-State (in-memory, UDP-Sessions)
    pub voice_state: VoiceState,
    /// Channel-Router (Voice-Pakete weiterleiten)
    pub channel_router: ChannelRouter,
    /// Presence-Manager (Wer ist online, in welchem Channel)
    pub presence: PresenceManager,
    /// Event-Broadcaster (Nachrichten an Clients senden)
    pub broadcaster: EventBroadcaster,
    /// Startzeitpunkt des Servers (fuer Uptime-Berechnung)
    pub start_time: Instant,
}

impl<U, P, B> SignalingState<U, P, B>
where
    U: UserRepository + ServerGroupRepository + ChannelRepository + ChatMessageRepository + 'static,
    P: PermissionRepository + 'static,
    B: BanRepository + 'static,
{
    /// Erstellt einen neuen SignalingState
    pub fn neu(
        config: SignalingConfig,
        auth_service: Arc<AuthService<U>>,
        permission_service: Arc<PermissionService<P>>,
        ban_service: Arc<BanService<B>>,
        db: Arc<U>,
        chat_service: Arc<ChatService<U>>,
    ) -> Arc<Self> {
        Arc::new(Self {
            config: Arc::new(config),
            auth_service,
            permission_service,
            ban_service,
            db,
            chat_service,
            voice_state: VoiceState::neu(),
            channel_router: ChannelRouter::neu(),
            presence: PresenceManager::neu(),
            broadcaster: EventBroadcaster::neu(),
            start_time: Instant::now(),
        })
    }

    /// Gibt die Uptime in Sekunden zurueck
    pub fn uptime_sek(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }
}
