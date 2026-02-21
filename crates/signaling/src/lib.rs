//! speakeasy-signaling – TCP/TLS Control Layer
//!
//! Dieser Crate implementiert den Signaling- und Session-Service fuer
//! Speakeasy. Er verwaltet TCP-Verbindungen, Authentifizierung, Channel-
//! Verwaltung und koordiniert Voice-Setup (UDP Port Negotiation).
//!
//! ## Architektur
//!
//! ```text
//! TCP Listener (SignalingServer)
//!     |
//!     v
//! ClientConnection (pro Verbindung ein Task)
//!     |  State Machine: Connected -> Authenticating -> Authenticated -> InChannel
//!     |
//!     v
//! MessageDispatcher
//!     |
//!     +-- AuthHandler      (Login, Logout, Session)
//!     +-- ChannelHandler   (Join, Leave, Create, Delete, Edit)
//!     +-- ClientHandler    (List, Kick, Ban, Move, Poke)
//!     +-- ServerHandler    (Info, Edit, Stop)
//!     +-- VoiceHandler     (Init, Ready, Disconnect)
//!     +-- PermissionHandler (List, Add, Remove)
//!
//! PresenceManager  – Wer ist online, in welchem Channel
//! EventBroadcaster – Events an alle relevanten Clients senden
//! ```

pub mod broadcast;
pub mod connection;
pub mod dispatcher;
pub mod error;
pub mod handlers;
pub mod presence;
pub mod server_state;
pub mod tcp;

// Bequeme Re-Exporte
pub use broadcast::EventBroadcaster;
pub use connection::ClientConnection;
pub use dispatcher::MessageDispatcher;
pub use error::{SignalingError, SignalingResult};
pub use presence::PresenceManager;
pub use tcp::SignalingServer;
