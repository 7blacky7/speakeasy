//! speakeasy-auth – Auth- und Permission-Service
//!
//! Dieses Crate implementiert:
//! - Passwort-Hashing mit Argon2id
//! - Session-Management (in-memory mit TTL)
//! - API-Token-Management mit Scopes
//! - AuthService (Registrierung, Login, Logout, Passwortwechsel)
//! - PermissionService (Berechtigungspruefung mit Caching)
//! - BanService (Benutzer- und IP-Bans)
//! - InviteService (Einladungscodes)

pub mod api_token;
pub mod ban_service;
pub mod error;
pub mod invite_service;
pub mod password;
pub mod permission_service;
pub mod service;
pub mod session;

// Bequeme Re-Exporte
pub use api_token::{ApiTokenRecord, ApiTokenStore, ErstellterApiToken, NeuesApiToken};
pub use ban_service::BanService;
pub use error::{AuthError, AuthResult};
pub use invite_service::InviteService;
pub use password::{passwort_hashen, passwort_verifizieren};
pub use permission_service::PermissionService;
pub use service::AuthService;
pub use session::{Session, SessionStore};
