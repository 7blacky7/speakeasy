//! Gemeinsame Identifikationstypen fuer Speakeasy
//!
//! Alle IDs verwenden das Newtype-Pattern um Verwechslungen zwischen
//! verschiedenen ID-Arten zur Compilezeit auszuschliessen.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Eindeutige Benutzer-ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UserId(pub Uuid);

impl UserId {
    /// Erstellt eine neue zufaellige UserId
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Gibt die innere UUID zurueck
    pub fn inner(&self) -> Uuid {
        self.0
    }
}

impl Default for UserId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for UserId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "user:{}", self.0)
    }
}

/// Eindeutige Kanal-ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChannelId(pub Uuid);

impl ChannelId {
    /// Erstellt eine neue zufaellige ChannelId
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Gibt die innere UUID zurueck
    pub fn inner(&self) -> Uuid {
        self.0
    }
}

impl Default for ChannelId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ChannelId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "channel:{}", self.0)
    }
}

/// Eindeutige Server-ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ServerId(pub Uuid);

impl ServerId {
    /// Erstellt eine neue zufaellige ServerId
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Gibt die innere UUID zurueck
    pub fn inner(&self) -> Uuid {
        self.0
    }
}

impl Default for ServerId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ServerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "server:{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_id_eindeutig() {
        let a = UserId::new();
        let b = UserId::new();
        assert_ne!(a, b, "Zwei neue UserIds muessen verschieden sein");
    }

    #[test]
    fn channel_id_eindeutig() {
        let a = ChannelId::new();
        let b = ChannelId::new();
        assert_ne!(a, b);
    }

    #[test]
    fn server_id_display() {
        let id = ServerId(Uuid::nil());
        assert!(id.to_string().starts_with("server:"));
    }

    #[test]
    fn ids_sind_serde_kompatibel() {
        let uid = UserId::new();
        let json = serde_json::to_string(&uid).unwrap();
        let uid2: UserId = serde_json::from_str(&json).unwrap();
        assert_eq!(uid, uid2);
    }
}
