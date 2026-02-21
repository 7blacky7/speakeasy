//! Control-Protokoll (TCP/TLS)
//!
//! Definiert alle Steuerungsnachrichten die ueber die TCP/TLS-Verbindung
//! zwischen Client und Server ausgetauscht werden.
//!
//! ## Design
//! - Request/Response Pattern: jede Nachricht hat eine `request_id: u32`
//! - JSON-Serialisierung via serde (TCP, nicht zeitkritisch)
//! - Tagged Enums fuer typsichere Nachrichtentypen

use serde::{Deserialize, Serialize};
use speakeasy_core::types::{ChannelId, ServerId, UserId};

// ---------------------------------------------------------------------------
// Fehler-Codes
// ---------------------------------------------------------------------------

/// Standardisierte Fehler-Codes fuer Error-Responses
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    // Allgemein
    InternalError,
    InvalidRequest,
    NotFound,
    PermissionDenied,
    RateLimited,
    // Auth
    InvalidCredentials,
    SessionExpired,
    AlreadyLoggedIn,
    // Channel
    ChannelFull,
    ChannelPasswordRequired,
    // Server
    ServerFull,
    Banned,
}

// ---------------------------------------------------------------------------
// Auth-Nachrichten
// ---------------------------------------------------------------------------

/// Login-Anfrage vom Client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRequest {
    /// Benutzername
    pub username: String,
    /// Passwort (Klartext – wird serverseitig gehasht verglichen)
    pub password: String,
    /// Optionaler API-Token statt Passwort
    pub token: Option<String>,
    /// Client-Version fuer Kompatibilitaetspruefung
    pub client_version: String,
    /// Anzeigename (kann vom Username abweichen)
    pub display_name: Option<String>,
}

/// Erfolgreiche Login-Antwort
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginResponse {
    /// Zugewiesene User-ID
    pub user_id: UserId,
    /// Session-Token fuer weitere Anfragen
    pub session_token: String,
    /// Server-ID
    pub server_id: ServerId,
    /// Ablaufzeit des Session-Tokens (Unix-Timestamp)
    pub expires_at: u64,
    /// Zugewiesene Server-Gruppen
    pub server_groups: Vec<String>,
}

/// Logout-Anfrage (Client trennt Verbindung sauber)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogoutRequest {
    pub reason: Option<String>,
}

/// Logout-Bestaetigung
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogoutResponse {
    pub success: bool,
}

// ---------------------------------------------------------------------------
// Channel-Nachrichten
// ---------------------------------------------------------------------------

/// Kanal-Informationen
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelInfo {
    pub channel_id: ChannelId,
    pub name: String,
    pub description: Option<String>,
    pub parent_id: Option<ChannelId>,
    pub sort_order: i32,
    pub max_clients: Option<u32>,
    pub current_clients: u32,
    pub password_protected: bool,
    pub codec: String,
    pub codec_quality: u8,
}

/// Liste aller Kanaele
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelListResponse {
    pub channels: Vec<ChannelInfo>,
}

/// Kanal beitreten
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelJoinRequest {
    pub channel_id: ChannelId,
    pub password: Option<String>,
}

/// Bestaetigung des Kanal-Beitritts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelJoinResponse {
    pub channel_id: ChannelId,
    /// Andere Clients im Kanal
    pub clients: Vec<ClientInfo>,
}

/// Kanal verlassen
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelLeaveRequest {
    pub channel_id: ChannelId,
}

/// Kanal erstellen
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelCreateRequest {
    pub name: String,
    pub description: Option<String>,
    pub parent_id: Option<ChannelId>,
    pub password: Option<String>,
    pub max_clients: Option<u32>,
    pub sort_order: Option<i32>,
}

/// Antwort auf Kanal-Erstellung
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelCreateResponse {
    pub channel_id: ChannelId,
}

/// Kanal bearbeiten
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelEditRequest {
    pub channel_id: ChannelId,
    pub name: Option<String>,
    pub description: Option<String>,
    pub password: Option<String>,
    pub max_clients: Option<u32>,
    pub sort_order: Option<i32>,
}

/// Kanal loeschen
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelDeleteRequest {
    pub channel_id: ChannelId,
    /// Ziel-Kanal fuer verbleibende Clients (None = Server-Root)
    pub move_clients_to: Option<ChannelId>,
}

// ---------------------------------------------------------------------------
// Client-Nachrichten
// ---------------------------------------------------------------------------

/// Client-Informationen
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub user_id: UserId,
    pub username: String,
    pub display_name: String,
    pub channel_id: Option<ChannelId>,
    pub server_groups: Vec<String>,
    pub is_muted: bool,
    pub is_deafened: bool,
    pub is_input_muted: bool,
}

/// Liste aller verbundenen Clients
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientListResponse {
    pub clients: Vec<ClientInfo>,
}

/// Client kicken
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientKickRequest {
    pub target_user_id: UserId,
    pub reason: Option<String>,
    pub from_channel_only: bool,
}

/// Client bannen
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientBanRequest {
    pub target_user_id: UserId,
    pub reason: Option<String>,
    /// Bann-Dauer in Sekunden (None = dauerhaft)
    pub duration_secs: Option<u64>,
    pub ban_ip: bool,
}

/// Client in anderen Kanal verschieben
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientMoveRequest {
    pub target_user_id: UserId,
    pub target_channel_id: ChannelId,
    pub reason: Option<String>,
}

/// Client anklopfen (Poke)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientPokeRequest {
    pub target_user_id: UserId,
    pub message: String,
}

/// Eigene Client-Informationen aktualisieren
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientUpdateRequest {
    pub display_name: Option<String>,
    pub is_input_muted: Option<bool>,
    pub is_output_muted: Option<bool>,
}

// ---------------------------------------------------------------------------
// Server-Nachrichten
// ---------------------------------------------------------------------------

/// Server-Informationen
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfoResponse {
    pub server_id: ServerId,
    pub name: String,
    pub welcome_message: Option<String>,
    pub max_clients: u32,
    pub current_clients: u32,
    pub version: String,
    pub uptime_secs: u64,
    pub host_message: Option<String>,
}

/// Server-Konfiguration bearbeiten
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerEditRequest {
    pub name: Option<String>,
    pub welcome_message: Option<String>,
    pub max_clients: Option<u32>,
    pub host_message: Option<String>,
}

/// Server herunterfahren (Admin)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerStopRequest {
    pub reason: Option<String>,
    /// Verzoegerung in Sekunden bevor der Server stoppt
    pub delay_secs: u32,
}

// ---------------------------------------------------------------------------
// Permission-Nachrichten
// ---------------------------------------------------------------------------

/// Permission-Eintrag
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionEntry {
    pub permission: String,
    pub value: PermissionValue,
}

/// Permission-Wert (TriState + optionale Limits)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionValue {
    Grant,
    Deny,
    Skip,
    IntLimit(i64),
}

/// Liste der Permissions fuer eine Gruppe/User
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionListResponse {
    pub target: String,
    pub permissions: Vec<PermissionEntry>,
}

/// Permission hinzufuegen
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionAddRequest {
    pub target: String,
    pub permission: String,
    pub value: PermissionValue,
}

/// Permission entfernen
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRemoveRequest {
    pub target: String,
    pub permission: String,
}

// ---------------------------------------------------------------------------
// File-Nachrichten
// ---------------------------------------------------------------------------

/// Datei-Eintrag
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub file_id: String,
    pub name: String,
    pub size_bytes: u64,
    pub channel_id: ChannelId,
    pub uploaded_by: UserId,
    pub uploaded_at: u64,
    pub mime_type: Option<String>,
}

/// Dateiliste eines Kanals
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileListResponse {
    pub channel_id: ChannelId,
    pub files: Vec<FileEntry>,
}

/// Datei-Upload initiieren
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileUploadRequest {
    pub channel_id: ChannelId,
    pub filename: String,
    pub size_bytes: u64,
    pub mime_type: Option<String>,
    /// Checksum (SHA-256 hex) fuer Integritaetspruefung
    pub checksum: Option<String>,
}

/// Upload-Token erhalten
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileUploadResponse {
    pub file_id: String,
    /// Upload-URL (HTTP/S3)
    pub upload_url: String,
    /// Token gueltigkeit in Sekunden
    pub expires_in_secs: u64,
}

/// Datei loeschen
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDeleteRequest {
    pub file_id: String,
}

// ---------------------------------------------------------------------------
// Voice-Setup (UDP Port Negotiation)
// ---------------------------------------------------------------------------

/// Voice-Verbindung initialisieren (UDP Port Negotiation)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceInitRequest {
    /// UDP-Port des Clients (fuer STUN/ICE)
    pub client_udp_port: u16,
    /// Bevorzugter Codec (wird bestaetigt oder abgelehnt)
    pub preferred_codec: String,
    /// DTLS-Fingerprint des Clients (fuer DTLS-Handshake)
    pub dtls_fingerprint: Option<String>,
}

/// Voice-Setup Bestaetigung vom Server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceReadyResponse {
    /// UDP-Port des Servers
    pub server_udp_port: u16,
    /// Server-IP (fuer direkte Verbindung)
    pub server_ip: String,
    /// Zugewiesene SSRC fuer diesen Client
    pub ssrc: u32,
    /// Akzeptierter Codec
    pub codec: String,
    /// DTLS-Fingerprint des Servers
    pub server_dtls_fingerprint: Option<String>,
    /// Krypto-Modus
    pub crypto_mode: String,
}

/// Voice-Verbindung trennen
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceDisconnectRequest {
    pub reason: Option<String>,
}

// ---------------------------------------------------------------------------
// Keepalive
// ---------------------------------------------------------------------------

/// Ping (Client -> Server oder Server -> Client)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PingMessage {
    /// Unix-Timestamp in Millisekunden fuer RTT-Messung
    pub timestamp_ms: u64,
}

/// Pong-Antwort (spiegelt Timestamp zurueck)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PongMessage {
    /// Originaler Timestamp aus dem Ping
    pub echo_timestamp_ms: u64,
    /// Server-eigener Timestamp
    pub server_timestamp_ms: u64,
}

// ---------------------------------------------------------------------------
// Haupt-Enum: ControlMessage
// ---------------------------------------------------------------------------

/// Alle moeglichen Control-Nachrichten (typsicher via Tagged Enum)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ControlPayload {
    // Auth
    Login(LoginRequest),
    LoginResponse(LoginResponse),
    Logout(LogoutRequest),
    LogoutResponse(LogoutResponse),

    // Channel
    ChannelList,
    ChannelListResponse(ChannelListResponse),
    ChannelJoin(ChannelJoinRequest),
    ChannelJoinResponse(ChannelJoinResponse),
    ChannelLeave(ChannelLeaveRequest),
    ChannelCreate(ChannelCreateRequest),
    ChannelCreateResponse(ChannelCreateResponse),
    ChannelEdit(ChannelEditRequest),
    ChannelDelete(ChannelDeleteRequest),

    // Client
    ClientList,
    ClientListResponse(ClientListResponse),
    ClientKick(ClientKickRequest),
    ClientBan(ClientBanRequest),
    ClientMove(ClientMoveRequest),
    ClientPoke(ClientPokeRequest),
    ClientUpdate(ClientUpdateRequest),

    // Server
    ServerInfo,
    ServerInfoResponse(ServerInfoResponse),
    ServerEdit(ServerEditRequest),
    ServerStop(ServerStopRequest),

    // Permission
    PermissionList { target: String },
    PermissionListResponse(PermissionListResponse),
    PermissionAdd(PermissionAddRequest),
    PermissionRemove(PermissionRemoveRequest),

    // File
    FileList { channel_id: ChannelId },
    FileListResponse(FileListResponse),
    FileUpload(FileUploadRequest),
    FileUploadResponse(FileUploadResponse),
    FileDelete(FileDeleteRequest),

    // Voice Setup
    VoiceInit(VoiceInitRequest),
    VoiceReady(VoiceReadyResponse),
    VoiceDisconnect(VoiceDisconnectRequest),

    // Keepalive
    Ping(PingMessage),
    Pong(PongMessage),

    // Error
    Error(ErrorResponse),
}

/// Standardisierte Fehler-Antwort
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub code: ErrorCode,
    pub message: String,
    /// Optionale maschinenlesbare Details
    pub details: Option<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Control-Frame (Umschlag fuer alle Nachrichten)
// ---------------------------------------------------------------------------

/// Control-Protokoll-Nachricht mit Request/Response-Zuordnung
///
/// Jede Nachricht traegt eine `request_id` die der Client vergibt.
/// Der Server kopiert die ID in die Antwort damit der Client
/// Request und Response zuordnen kann.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlMessage {
    /// Eindeutige Nachrichten-ID fuer Request/Response-Zuordnung
    pub request_id: u32,
    /// Inhalt der Nachricht
    pub payload: ControlPayload,
}

impl ControlMessage {
    /// Erstellt eine neue Control-Nachricht
    pub fn new(request_id: u32, payload: ControlPayload) -> Self {
        Self {
            request_id,
            payload,
        }
    }

    /// Erstellt eine Ping-Nachricht
    pub fn ping(request_id: u32, timestamp_ms: u64) -> Self {
        Self::new(
            request_id,
            ControlPayload::Ping(PingMessage { timestamp_ms }),
        )
    }

    /// Erstellt eine Pong-Antwort
    pub fn pong(request_id: u32, echo_timestamp_ms: u64, server_timestamp_ms: u64) -> Self {
        Self::new(
            request_id,
            ControlPayload::Pong(PongMessage {
                echo_timestamp_ms,
                server_timestamp_ms,
            }),
        )
    }

    /// Erstellt eine Fehler-Antwort
    pub fn error(request_id: u32, code: ErrorCode, message: impl Into<String>) -> Self {
        Self::new(
            request_id,
            ControlPayload::Error(ErrorResponse {
                code,
                message: message.into(),
                details: None,
            }),
        )
    }

    /// Serialisiert die Nachricht als JSON
    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string(self)
    }

    /// Deserialisiert eine Nachricht aus JSON
    pub fn from_json(json: &str) -> serde_json::Result<Self> {
        serde_json::from_str(json)
    }
}

// ---------------------------------------------------------------------------
// Rueckwaertskompatibilitaet Phase 1 (altes Control-Protokoll)
// ---------------------------------------------------------------------------

/// Typ einer Protokoll-Nachricht (Phase 1, deprecated)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageType {
    Anfrage,
    Antwort,
    Benachrichtigung,
    Fehler,
}

/// Befehle (Phase 1, deprecated)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommandType {
    Handshake,
    Ping,
    Pong,
    Trennen,
    Login,
    Logout,
    TokenErneuern,
    KanalListe,
    KanalBetreten,
    KanalVerlassen,
    KanalErstellen,
    KanalLoeschen,
    BenutzerInfo,
    BenutzerAktualisieren,
    ServerInfo,
    Kicken,
    Bannen,
}

/// Protokollversion (Phase 1)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtokollVersion {
    pub major: u16,
    pub minor: u16,
}

impl ProtokollVersion {
    pub const AKTUELL: Self = Self { major: 1, minor: 0 };
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ping_pong_serialisierung() {
        let ping = ControlMessage::ping(1, 1234567890);
        let json = ping.to_json().unwrap();
        let decoded = ControlMessage::from_json(&json).unwrap();
        assert_eq!(decoded.request_id, 1);
        if let ControlPayload::Ping(p) = decoded.payload {
            assert_eq!(p.timestamp_ms, 1234567890);
        } else {
            panic!("Erwartet Ping-Payload");
        }
    }

    #[test]
    fn error_response_serialisierung() {
        let msg = ControlMessage::error(42, ErrorCode::PermissionDenied, "Keine Berechtigung");
        let json = msg.to_json().unwrap();
        let decoded = ControlMessage::from_json(&json).unwrap();
        assert_eq!(decoded.request_id, 42);
        if let ControlPayload::Error(e) = decoded.payload {
            assert_eq!(e.code, ErrorCode::PermissionDenied);
            assert_eq!(e.message, "Keine Berechtigung");
        } else {
            panic!("Erwartet Error-Payload");
        }
    }

    #[test]
    fn login_request_serialisierung() {
        let req = ControlMessage::new(
            5,
            ControlPayload::Login(LoginRequest {
                username: "testuser".to_string(),
                password: "secret".to_string(),
                token: None,
                client_version: "1.0.0".to_string(),
                display_name: Some("Test User".to_string()),
            }),
        );
        let json = req.to_json().unwrap();
        let decoded = ControlMessage::from_json(&json).unwrap();
        assert_eq!(decoded.request_id, 5);
        if let ControlPayload::Login(l) = decoded.payload {
            assert_eq!(l.username, "testuser");
        } else {
            panic!("Erwartet Login-Payload");
        }
    }

    #[test]
    fn channel_list_request_serialisierung() {
        let msg = ControlMessage::new(10, ControlPayload::ChannelList);
        let json = msg.to_json().unwrap();
        let decoded = ControlMessage::from_json(&json).unwrap();
        assert_eq!(decoded.request_id, 10);
        assert!(matches!(decoded.payload, ControlPayload::ChannelList));
    }

    #[test]
    fn voice_init_serialisierung() {
        let req = ControlMessage::new(
            20,
            ControlPayload::VoiceInit(VoiceInitRequest {
                client_udp_port: 4444,
                preferred_codec: "opus".to_string(),
                dtls_fingerprint: Some("AA:BB:CC".to_string()),
            }),
        );
        let json = req.to_json().unwrap();
        let decoded = ControlMessage::from_json(&json).unwrap();
        if let ControlPayload::VoiceInit(v) = decoded.payload {
            assert_eq!(v.client_udp_port, 4444);
            assert_eq!(v.preferred_codec, "opus");
        } else {
            panic!("Erwartet VoiceInit-Payload");
        }
    }

    #[test]
    fn permission_value_alle_varianten() {
        let grant = PermissionValue::Grant;
        let deny = PermissionValue::Deny;
        let skip = PermissionValue::Skip;
        let limit = PermissionValue::IntLimit(100);

        let json_g = serde_json::to_string(&grant).unwrap();
        let json_d = serde_json::to_string(&deny).unwrap();
        let json_s = serde_json::to_string(&skip).unwrap();
        let json_l = serde_json::to_string(&limit).unwrap();

        let _: PermissionValue = serde_json::from_str(&json_g).unwrap();
        let _: PermissionValue = serde_json::from_str(&json_d).unwrap();
        let _: PermissionValue = serde_json::from_str(&json_s).unwrap();
        let decoded_limit: PermissionValue = serde_json::from_str(&json_l).unwrap();
        assert!(matches!(decoded_limit, PermissionValue::IntLimit(100)));
    }

    #[test]
    fn client_ban_request_serialisierung() {
        let uid = UserId::new();
        let req = ControlMessage::new(
            30,
            ControlPayload::ClientBan(ClientBanRequest {
                target_user_id: uid,
                reason: Some("Regelverstoß".to_string()),
                duration_secs: Some(3600),
                ban_ip: true,
            }),
        );
        let json = req.to_json().unwrap();
        let decoded = ControlMessage::from_json(&json).unwrap();
        if let ControlPayload::ClientBan(b) = decoded.payload {
            assert_eq!(b.target_user_id, uid);
            assert_eq!(b.duration_secs, Some(3600));
            assert!(b.ban_ip);
        } else {
            panic!("Erwartet ClientBan-Payload");
        }
    }

    #[test]
    fn error_codes_serialisierbar() {
        let codes = [
            ErrorCode::InternalError,
            ErrorCode::InvalidCredentials,
            ErrorCode::ChannelFull,
            ErrorCode::Banned,
        ];
        for code in &codes {
            let json = serde_json::to_string(code).unwrap();
            let decoded: ErrorCode = serde_json::from_str(&json).unwrap();
            assert_eq!(*code, decoded);
        }
    }
}
