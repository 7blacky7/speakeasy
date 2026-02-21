//! Session-Management fuer Speakeasy
//!
//! Implementiert kurzlebige Session-Tokens fuer eingeloggte Benutzer.
//! Sessions werden im Speicher gehalten (in-memory HashMap mit TTL).
//! Ein Hintergrund-Task bereinigt abgelaufene Sessions automatisch.

use std::{
    collections::HashMap,
    sync::Arc,
    time::Duration,
};

use chrono::{DateTime, Utc};
use rand::RngCore;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::error::{AuthError, AuthResult};

/// Standard-Session-Lebensdauer: 24 Stunden
const SESSION_TTL_SEKUNDEN: i64 = 24 * 60 * 60;

/// Intervall fuer den automatischen Cleanup-Task: 15 Minuten
const CLEANUP_INTERVALL: Duration = Duration::from_secs(15 * 60);

/// Ein aktives Session-Token
#[derive(Debug, Clone)]
pub struct Session {
    /// Der Token-String (URL-sicheres Base64)
    pub token: String,
    /// ID des Benutzers dem diese Session gehoert
    pub user_id: Uuid,
    /// Zeitpunkt der Session-Erstellung
    pub erstellt_am: DateTime<Utc>,
    /// Zeitpunkt des Session-Ablaufs
    pub laeuft_ab_am: DateTime<Utc>,
}

impl Session {
    /// Gibt `true` zurueck wenn die Session noch gueltig ist
    pub fn ist_gueltig(&self) -> bool {
        Utc::now() < self.laeuft_ab_am
    }
}

/// In-Memory Session-Store mit TTL-Unterstuetzung
#[derive(Debug, Default)]
pub struct SessionStore {
    /// token -> Session
    sessions: RwLock<HashMap<String, Session>>,
}

impl SessionStore {
    /// Erstellt einen neuen leeren Session-Store
    pub fn neu() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Erstellt einen neuen Session-Store und startet den Cleanup-Task
    pub fn neu_mit_cleanup(store: Arc<Self>) -> Arc<Self> {
        let store_klon = Arc::clone(&store);
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(CLEANUP_INTERVALL).await;
                let entfernt = store_klon.cleanup_abgelaufene().await;
                if entfernt > 0 {
                    tracing::debug!(anzahl = entfernt, "Abgelaufene Sessions bereinigt");
                }
            }
        });
        store
    }

    /// Erstellt eine neue Session fuer den angegebenen Benutzer
    ///
    /// Gibt den generierten Session-Token zurueck.
    pub async fn erstellen(&self, user_id: Uuid) -> AuthResult<Session> {
        let token = token_generieren();
        let jetzt = Utc::now();
        let session = Session {
            token: token.clone(),
            user_id,
            erstellt_am: jetzt,
            laeuft_ab_am: jetzt + chrono::Duration::seconds(SESSION_TTL_SEKUNDEN),
        };

        self.sessions.write().await.insert(token, session.clone());
        tracing::debug!(user_id = %user_id, "Neue Session erstellt");
        Ok(session)
    }

    /// Validiert einen Session-Token und gibt die Session zurueck
    ///
    /// Gibt `AuthError::SessionUngueltig` zurueck wenn der Token nicht gefunden wurde.
    /// Gibt `AuthError::SessionAbgelaufen` zurueck wenn die Session abgelaufen ist.
    pub async fn validieren(&self, token: &str) -> AuthResult<Session> {
        let sessions = self.sessions.read().await;
        match sessions.get(token) {
            None => Err(AuthError::SessionUngueltig),
            Some(session) if !session.ist_gueltig() => Err(AuthError::SessionAbgelaufen),
            Some(session) => Ok(session.clone()),
        }
    }

    /// Invalidiert (loescht) eine Session anhand des Tokens
    pub async fn invalidieren(&self, token: &str) -> AuthResult<()> {
        let mut sessions = self.sessions.write().await;
        sessions.remove(token);
        tracing::debug!("Session invalidiert");
        Ok(())
    }

    /// Invalidiert alle Sessions eines Benutzers (z.B. bei Passwortaenderung)
    pub async fn alle_invalidieren(&self, user_id: Uuid) -> usize {
        let mut sessions = self.sessions.write().await;
        let vorher = sessions.len();
        sessions.retain(|_, s| s.user_id != user_id);
        let entfernt = vorher - sessions.len();
        if entfernt > 0 {
            tracing::debug!(user_id = %user_id, anzahl = entfernt, "Alle User-Sessions invalidiert");
        }
        entfernt
    }

    /// Bereinigt abgelaufene Sessions und gibt die Anzahl der entfernten Sessions zurueck
    pub async fn cleanup_abgelaufene(&self) -> usize {
        let jetzt = Utc::now();
        let mut sessions = self.sessions.write().await;
        let vorher = sessions.len();
        sessions.retain(|_, s| s.laeuft_ab_am > jetzt);
        vorher - sessions.len()
    }

    /// Gibt die Anzahl der aktiven (nicht abgelaufenen) Sessions zurueck
    pub async fn anzahl_aktive(&self) -> usize {
        let jetzt = Utc::now();
        let sessions = self.sessions.read().await;
        sessions.values().filter(|s| s.laeuft_ab_am > jetzt).count()
    }
}

/// Generiert einen kryptografisch sicheren Session-Token (URL-sicheres Base64)
fn token_generieren() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn session_erstellen_und_validieren() {
        let store = SessionStore::neu();
        let user_id = Uuid::new_v4();

        let session = store.erstellen(user_id).await.expect("Session-Erstellung fehlgeschlagen");
        assert_eq!(session.user_id, user_id);
        assert!(session.ist_gueltig());

        let validiert = store.validieren(&session.token).await.expect("Validierung fehlgeschlagen");
        assert_eq!(validiert.user_id, user_id);
    }

    #[tokio::test]
    async fn ungueltige_session_gibt_fehler() {
        let store = SessionStore::neu();
        let ergebnis = store.validieren("kein_gueltiger_token").await;
        assert!(matches!(ergebnis, Err(AuthError::SessionUngueltig)));
    }

    #[tokio::test]
    async fn session_invalidieren() {
        let store = SessionStore::neu();
        let user_id = Uuid::new_v4();
        let session = store.erstellen(user_id).await.unwrap();

        store.invalidieren(&session.token).await.unwrap();
        let ergebnis = store.validieren(&session.token).await;
        assert!(matches!(ergebnis, Err(AuthError::SessionUngueltig)));
    }

    #[tokio::test]
    async fn alle_user_sessions_invalidieren() {
        let store = SessionStore::neu();
        let user_id = Uuid::new_v4();

        let _s1 = store.erstellen(user_id).await.unwrap();
        let _s2 = store.erstellen(user_id).await.unwrap();
        let _s3 = store.erstellen(Uuid::new_v4()).await.unwrap();

        let entfernt = store.alle_invalidieren(user_id).await;
        assert_eq!(entfernt, 2);
        assert_eq!(store.anzahl_aktive().await, 1);
    }

    #[tokio::test]
    async fn token_sind_eindeutig() {
        let store = SessionStore::neu();
        let user_id = Uuid::new_v4();

        let s1 = store.erstellen(user_id).await.unwrap();
        let s2 = store.erstellen(user_id).await.unwrap();
        assert_ne!(s1.token, s2.token, "Session-Tokens muessen eindeutig sein");
    }
}
