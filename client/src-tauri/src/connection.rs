//! Client-seitige TCP-Verbindung zum Speakeasy-Server
//!
//! Nutzt den FrameCodec aus speakeasy-protocol fuer das Wire-Format
//! (u32 BE length + JSON payload). Alle Operationen sind async.

use futures_util::{SinkExt, StreamExt};
use speakeasy_protocol::{
    control::{
        ChannelJoinRequest, ChannelLeaveRequest, ControlMessage, ControlPayload, ErrorCode,
        ErrorResponse, LoginRequest, LoginResponse, LogoutRequest, ServerInfoResponse,
    },
    wire::FrameCodec,
};
use std::sync::atomic::{AtomicU32, Ordering};
use tokio::net::TcpStream;
use tokio_util::codec::Framed;

// ---------------------------------------------------------------------------
// Fehler-Typ
// ---------------------------------------------------------------------------

/// Fehler die bei der Server-Verbindung auftreten koennen
#[derive(Debug)]
pub enum ConnectionError {
    /// TCP-Verbindung fehlgeschlagen
    Io(std::io::Error),
    /// Server hat mit Fehler geantwortet
    ServerError { code: ErrorCode, message: String },
    /// Unerwartete Antwort vom Server
    UnexpectedResponse(String),
    /// Nicht verbunden
    NotConnected,
}

impl std::fmt::Display for ConnectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectionError::Io(e) => write!(f, "IO-Fehler: {}", e),
            ConnectionError::ServerError { code, message } => {
                write!(f, "Server-Fehler ({:?}): {}", code, message)
            }
            ConnectionError::UnexpectedResponse(msg) => {
                write!(f, "Unerwartete Antwort: {}", msg)
            }
            ConnectionError::NotConnected => write!(f, "Nicht mit Server verbunden"),
        }
    }
}

impl From<std::io::Error> for ConnectionError {
    fn from(e: std::io::Error) -> Self {
        ConnectionError::Io(e)
    }
}

impl From<ConnectionError> for String {
    fn from(e: ConnectionError) -> Self {
        e.to_string()
    }
}

// ---------------------------------------------------------------------------
// ServerConnection
// ---------------------------------------------------------------------------

/// Echte TCP-Verbindung zum Speakeasy Signaling-Server
pub struct ServerConnection {
    /// Framed TCP-Stream mit FrameCodec
    framed: Framed<TcpStream, FrameCodec>,
    /// Session-Token nach erfolgreichem Login
    session_token: Option<String>,
    /// Eigene User-ID nach Login
    user_id: Option<String>,
    /// Monoton steigender Request-ID Zaehler
    next_request_id: AtomicU32,
}

impl ServerConnection {
    /// Baut eine TCP-Verbindung zum Server auf
    pub async fn connect(addr: &str, port: u16) -> Result<Self, ConnectionError> {
        let address = format!("{}:{}", addr, port);
        tracing::info!("Verbinde mit {}", address);
        let stream = TcpStream::connect(&address).await?;
        tracing::info!("TCP-Verbindung hergestellt zu {}", address);

        let framed = Framed::new(stream, FrameCodec::new());

        Ok(Self {
            framed,
            session_token: None,
            user_id: None,
            next_request_id: AtomicU32::new(1),
        })
    }

    /// Generiert die naechste Request-ID
    pub fn next_id(&self) -> u32 {
        self.next_request_id.fetch_add(1, Ordering::Relaxed)
    }

    /// Sendet eine ControlMessage und wartet auf die Antwort
    pub async fn send_and_receive(
        &mut self,
        message: ControlMessage,
    ) -> Result<ControlMessage, ConnectionError> {
        self.framed.send(message).await?;

        // Auf Antwort warten (Pings vom Server automatisch beantworten)
        loop {
            match self.framed.next().await {
                Some(Ok(response)) => {
                    // Server-Ping automatisch beantworten
                    if let ControlPayload::Ping(ref ping) = response.payload {
                        let ts = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64;
                        let pong =
                            ControlMessage::pong(response.request_id, ping.timestamp_ms, ts);
                        self.framed.send(pong).await?;
                        continue;
                    }
                    return Ok(response);
                }
                Some(Err(e)) => return Err(ConnectionError::Io(e)),
                None => {
                    return Err(ConnectionError::Io(std::io::Error::new(
                        std::io::ErrorKind::ConnectionReset,
                        "Verbindung vom Server getrennt",
                    )))
                }
            }
        }
    }

    /// Prueft ob die Antwort ein Fehler ist und konvertiert ihn
    fn check_error(response: &ControlMessage) -> Result<(), ConnectionError> {
        if let ControlPayload::Error(ErrorResponse {
            code, message, ..
        }) = &response.payload
        {
            return Err(ConnectionError::ServerError {
                code: *code,
                message: message.clone(),
            });
        }
        Ok(())
    }

    /// Login am Server mit Benutzername und Passwort
    pub async fn login(
        &mut self,
        username: &str,
        password: &str,
    ) -> Result<LoginResponse, ConnectionError> {
        let request_id = self.next_id();
        let msg = ControlMessage::new(
            request_id,
            ControlPayload::Login(LoginRequest {
                username: username.to_string(),
                password: password.to_string(),
                token: None,
                client_version: env!("CARGO_PKG_VERSION").to_string(),
                display_name: None,
            }),
        );

        let response = self.send_and_receive(msg).await?;
        Self::check_error(&response)?;

        match response.payload {
            ControlPayload::LoginResponse(login_resp) => {
                self.session_token = Some(login_resp.session_token.clone());
                self.user_id = Some(login_resp.user_id.inner().to_string());
                tracing::info!(
                    "Login erfolgreich: user_id={}",
                    login_resp.user_id.inner()
                );
                Ok(login_resp)
            }
            other => Err(ConnectionError::UnexpectedResponse(format!(
                "Erwartet LoginResponse, erhalten: {:?}",
                std::mem::discriminant(&other)
            ))),
        }
    }

    /// Logout vom Server
    pub async fn logout(&mut self) -> Result<(), ConnectionError> {
        let request_id = self.next_id();
        let msg = ControlMessage::new(
            request_id,
            ControlPayload::Logout(LogoutRequest { reason: None }),
        );

        let response = self.send_and_receive(msg).await?;
        Self::check_error(&response)?;
        self.session_token = None;
        self.user_id = None;
        tracing::info!("Logout erfolgreich");
        Ok(())
    }

    /// Trennt die TCP-Verbindung
    pub async fn disconnect(&mut self) {
        // Versuche sauber zu senden, ignoriere Fehler
        let _ = self.framed.close().await;
        self.session_token = None;
        self.user_id = None;
        tracing::info!("TCP-Verbindung getrennt");
    }

    /// Kanal beitreten
    pub async fn join_channel(
        &mut self,
        channel_id: &str,
    ) -> Result<(), ConnectionError> {
        let request_id = self.next_id();
        let uuid = uuid::Uuid::parse_str(channel_id).map_err(|e| {
            ConnectionError::UnexpectedResponse(format!("Ungueltige Channel-ID: {}", e))
        })?;
        let cid = speakeasy_core::types::ChannelId(uuid);
        let msg = ControlMessage::new(
            request_id,
            ControlPayload::ChannelJoin(ChannelJoinRequest {
                channel_id: cid,
                password: None,
            }),
        );

        let response = self.send_and_receive(msg).await?;
        Self::check_error(&response)?;

        match response.payload {
            ControlPayload::ChannelJoinResponse(_) => {
                tracing::info!("Kanal {} beigetreten", channel_id);
                Ok(())
            }
            other => Err(ConnectionError::UnexpectedResponse(format!(
                "Erwartet ChannelJoinResponse, erhalten: {:?}",
                std::mem::discriminant(&other)
            ))),
        }
    }

    /// Kanal verlassen
    pub async fn leave_channel(
        &mut self,
        channel_id: &str,
    ) -> Result<(), ConnectionError> {
        let request_id = self.next_id();
        let uuid = uuid::Uuid::parse_str(channel_id).map_err(|e| {
            ConnectionError::UnexpectedResponse(format!("Ungueltige Channel-ID: {}", e))
        })?;
        let cid = speakeasy_core::types::ChannelId(uuid);
        let msg = ControlMessage::new(
            request_id,
            ControlPayload::ChannelLeave(ChannelLeaveRequest { channel_id: cid }),
        );

        let response = self.send_and_receive(msg).await?;
        Self::check_error(&response)?;
        tracing::info!("Kanal {} verlassen", channel_id);
        Ok(())
    }

    /// Server-Informationen abrufen
    pub async fn get_server_info(&mut self) -> Result<ServerInfoResponse, ConnectionError> {
        let request_id = self.next_id();
        let msg = ControlMessage::new(request_id, ControlPayload::ServerInfo);

        let response = self.send_and_receive(msg).await?;
        Self::check_error(&response)?;

        match response.payload {
            ControlPayload::ServerInfoResponse(info) => Ok(info),
            other => Err(ConnectionError::UnexpectedResponse(format!(
                "Erwartet ServerInfoResponse, erhalten: {:?}",
                std::mem::discriminant(&other)
            ))),
        }
    }

    /// Channel-Liste abrufen
    pub async fn get_channel_list(
        &mut self,
    ) -> Result<Vec<speakeasy_protocol::control::ChannelInfo>, ConnectionError> {
        let request_id = self.next_id();
        let msg = ControlMessage::new(request_id, ControlPayload::ChannelList);

        let response = self.send_and_receive(msg).await?;
        Self::check_error(&response)?;

        match response.payload {
            ControlPayload::ChannelListResponse(list) => Ok(list.channels),
            other => Err(ConnectionError::UnexpectedResponse(format!(
                "Erwartet ChannelListResponse, erhalten: {:?}",
                std::mem::discriminant(&other)
            ))),
        }
    }

    /// Session-Token zurueckgeben
    pub fn session_token(&self) -> Option<&str> {
        self.session_token.as_deref()
    }

    /// Eigene User-ID zurueckgeben
    pub fn user_id(&self) -> Option<&str> {
        self.user_id.as_deref()
    }
}
