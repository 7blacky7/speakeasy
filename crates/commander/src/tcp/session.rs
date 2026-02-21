//! TCP-Session-Management

use uuid::Uuid;

use crate::auth::CommanderSession;

/// Zustand einer TCP-Verbindung
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionZustand {
    /// Verbunden, aber noch nicht eingeloggt
    Unauthentifiziert,
    /// Eingeloggt
    Authentifiziert,
    /// Verbindung wird beendet
    Beendend,
}

/// Eine aktive TCP-Commander-Session
#[derive(Debug)]
pub struct TcpSession {
    pub id: Uuid,
    pub zustand: SessionZustand,
    pub commander_session: Option<CommanderSession>,
    pub client_addr: std::net::SocketAddr,
}

impl TcpSession {
    pub fn neu(client_addr: std::net::SocketAddr) -> Self {
        Self {
            id: Uuid::new_v4(),
            zustand: SessionZustand::Unauthentifiziert,
            commander_session: None,
            client_addr,
        }
    }

    pub fn ist_authentifiziert(&self) -> bool {
        self.zustand == SessionZustand::Authentifiziert
    }

    pub fn anmelden(&mut self, session: CommanderSession) {
        self.commander_session = Some(session);
        self.zustand = SessionZustand::Authentifiziert;
    }

    pub fn abmelden(&mut self) {
        self.commander_session = None;
        self.zustand = SessionZustand::Unauthentifiziert;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_addr() -> std::net::SocketAddr {
        "127.0.0.1:12345".parse().unwrap()
    }

    #[test]
    fn neue_session_ist_unauthentifiziert() {
        let session = TcpSession::neu(test_addr());
        assert!(!session.ist_authentifiziert());
        assert!(session.commander_session.is_none());
    }

    #[test]
    fn session_id_ist_eindeutig() {
        let s1 = TcpSession::neu(test_addr());
        let s2 = TcpSession::neu(test_addr());
        assert_ne!(s1.id, s2.id);
    }

    #[test]
    fn session_zustand_wechsel() {
        use chrono::Utc;
        use speakeasy_db::models::BenutzerRecord;
        use crate::auth::AuthArt;

        let mut session = TcpSession::neu(test_addr());
        assert_eq!(session.zustand, SessionZustand::Unauthentifiziert);

        let cmd_session = CommanderSession {
            benutzer: BenutzerRecord {
                id: Uuid::new_v4(),
                username: "admin".into(),
                password_hash: "".into(),
                created_at: Utc::now(),
                last_login: None,
                is_active: true,
            },
            scopes: vec![],
            auth_art: AuthArt::Session,
        };
        session.anmelden(cmd_session);
        assert!(session.ist_authentifiziert());
        assert!(session.commander_session.is_some());

        session.abmelden();
        assert!(!session.ist_authentifiziert());
    }
}
