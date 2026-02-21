//! Commander-Authentifizierung
//!
//! Validiert Session-Tokens und API-Tokens fuer alle drei Interfaces
//! (REST, TCP, gRPC).

use std::sync::Arc;

use speakeasy_auth::{AuthService, session::Session};
use speakeasy_db::{models::BenutzerRecord, repository::UserRepository};

use crate::error::{CommanderError, CommanderResult};

/// Identitaet einer authentifizierten Commander-Session
#[derive(Debug, Clone)]
pub struct CommanderSession {
    /// Der authentifizierte Benutzer
    pub benutzer: BenutzerRecord,
    /// Scopes des API-Tokens (leer bei Session-Auth)
    pub scopes: Vec<String>,
    /// Art der Authentifizierung
    pub auth_art: AuthArt,
}

/// Art der Commander-Authentifizierung
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthArt {
    /// Session-Token (kurzlebig, nach Login)
    Session,
    /// API-Token (langlebig, mit Scopes)
    ApiToken,
}

impl CommanderSession {
    /// Prueft ob die Session einen bestimmten Scope hat (fuer API-Tokens)
    pub fn hat_scope(&self, scope: &str) -> bool {
        match self.auth_art {
            // Session-Auth hat alle Rechte (wie Admin-Login)
            AuthArt::Session => true,
            // API-Token: Scope muss explizit vorhanden sein
            AuthArt::ApiToken => {
                self.scopes.iter().any(|s| s == scope || s == "admin:*")
            }
        }
    }
}

/// Commander-Auth-Service
///
/// Wrapper um den AuthService der speakeasy-auth-Crate fuer
/// Commander-spezifische Authentifizierung.
pub struct CommanderAuth<U: UserRepository> {
    auth_service: Arc<AuthService<U>>,
}

impl<U: UserRepository> CommanderAuth<U> {
    pub fn neu(auth_service: Arc<AuthService<U>>) -> Self {
        Self { auth_service }
    }

    /// Validiert einen Bearer-Token (Session oder API-Token)
    ///
    /// Erwartet Format: "Bearer <token>"
    pub async fn bearer_validieren(&self, authorization: &str) -> CommanderResult<CommanderSession> {
        let token = authorization
            .strip_prefix("Bearer ")
            .ok_or_else(|| CommanderError::Authentifizierung(
                "Ungueltiges Authorization-Format (erwartet: Bearer <token>)".into()
            ))?;

        self.token_validieren(token).await
    }

    /// Validiert einen Token-String direkt (ohne "Bearer "-Prefix)
    pub async fn token_validieren(&self, token: &str) -> CommanderResult<CommanderSession> {
        // Versuche zuerst als Session-Token
        if let Ok(benutzer) = self.auth_service.session_validieren(token).await {
            return Ok(CommanderSession {
                benutzer,
                scopes: vec![],
                auth_art: AuthArt::Session,
            });
        }

        // Dann als API-Token versuchen
        match self.auth_service.api_token_validieren(token).await {
            Ok((benutzer, scopes)) => Ok(CommanderSession {
                benutzer,
                scopes,
                auth_art: AuthArt::ApiToken,
            }),
            Err(_) => Err(CommanderError::Authentifizierung(
                "Ungueltiger oder abgelaufener Token".into()
            )),
        }
    }

    /// Login fuer TCP-Interface: Benutzername + Passwort -> Session-Token
    pub async fn anmelden(
        &self,
        username: &str,
        passwort: &str,
    ) -> CommanderResult<(BenutzerRecord, Session)> {
        self.auth_service
            .anmelden(username, passwort)
            .await
            .map_err(|_| CommanderError::Authentifizierung(
                "Ungueltige Anmeldedaten".into()
            ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use chrono::Utc;
    use uuid::Uuid;
    use speakeasy_auth::{ApiTokenStore, SessionStore};
    use speakeasy_db::{DbError, models::{BenutzerRecord, BenutzerUpdate, NeuerBenutzer}, repository::UserRepository};

    #[derive(Default)]
    struct TestUserRepo {
        benutzer: Mutex<Vec<BenutzerRecord>>,
    }

    impl UserRepository for TestUserRepo {
        async fn create(&self, data: NeuerBenutzer<'_>) -> speakeasy_db::DbResult<BenutzerRecord> {
            let record = BenutzerRecord {
                id: Uuid::new_v4(),
                username: data.username.to_string(),
                password_hash: data.password_hash.to_string(),
                created_at: Utc::now(),
                last_login: None,
                is_active: true,
            };
            self.benutzer.lock().unwrap().push(record.clone());
            Ok(record)
        }
        async fn get_by_id(&self, id: Uuid) -> speakeasy_db::DbResult<Option<BenutzerRecord>> {
            Ok(self.benutzer.lock().unwrap().iter().find(|u| u.id == id).cloned())
        }
        async fn get_by_name(&self, username: &str) -> speakeasy_db::DbResult<Option<BenutzerRecord>> {
            Ok(self.benutzer.lock().unwrap().iter().find(|u| u.username == username).cloned())
        }
        async fn update(&self, id: Uuid, _data: BenutzerUpdate) -> speakeasy_db::DbResult<BenutzerRecord> {
            self.get_by_id(id).await?.ok_or_else(|| DbError::nicht_gefunden(id.to_string()))
        }
        async fn delete(&self, _id: Uuid) -> speakeasy_db::DbResult<bool> { Ok(false) }
        async fn list(&self, _nur_aktive: bool) -> speakeasy_db::DbResult<Vec<BenutzerRecord>> {
            Ok(self.benutzer.lock().unwrap().clone())
        }
        async fn authenticate(&self, _u: &str, _p: &str) -> speakeasy_db::DbResult<Option<BenutzerRecord>> { Ok(None) }
        async fn update_last_login(&self, _id: Uuid) -> speakeasy_db::DbResult<()> { Ok(()) }
    }

    fn test_auth() -> CommanderAuth<TestUserRepo> {
        let repo = Arc::new(TestUserRepo::default());
        let sessions = SessionStore::neu();
        let tokens = ApiTokenStore::neu();
        let service = Arc::new(AuthService::neu(repo, sessions, tokens));
        CommanderAuth::neu(service)
    }

    #[tokio::test]
    async fn bearer_format_wird_geparst() {
        let auth = test_auth();
        let ergebnis = auth.bearer_validieren("kein_bearer_format").await;
        assert!(matches!(ergebnis, Err(CommanderError::Authentifizierung(_))));
    }

    #[tokio::test]
    async fn ungueltiger_token_abgelehnt() {
        let auth = test_auth();
        let ergebnis = auth.token_validieren("ungueltiger_token_xyz").await;
        assert!(matches!(ergebnis, Err(CommanderError::Authentifizierung(_))));
    }

    #[test]
    fn session_hat_alle_scopes() {
        let session = CommanderSession {
            benutzer: BenutzerRecord {
                id: Uuid::new_v4(),
                username: "test".into(),
                password_hash: "".into(),
                created_at: Utc::now(),
                last_login: None,
                is_active: true,
            },
            scopes: vec![],
            auth_art: AuthArt::Session,
        };
        assert!(session.hat_scope("admin:read"));
        assert!(session.hat_scope("admin:write"));
        assert!(session.hat_scope("irgendwas"));
    }

    #[test]
    fn api_token_scope_pruefung() {
        let session = CommanderSession {
            benutzer: BenutzerRecord {
                id: Uuid::new_v4(),
                username: "test".into(),
                password_hash: "".into(),
                created_at: Utc::now(),
                last_login: None,
                is_active: true,
            },
            scopes: vec!["admin:read".to_string()],
            auth_art: AuthArt::ApiToken,
        };
        assert!(session.hat_scope("admin:read"));
        assert!(!session.hat_scope("admin:write"));
    }

    #[test]
    fn api_token_wildcard_scope() {
        let session = CommanderSession {
            benutzer: BenutzerRecord {
                id: Uuid::new_v4(),
                username: "test".into(),
                password_hash: "".into(),
                created_at: Utc::now(),
                last_login: None,
                is_active: true,
            },
            scopes: vec!["admin:*".to_string()],
            auth_art: AuthArt::ApiToken,
        };
        assert!(session.hat_scope("admin:lesen"));
        assert!(session.hat_scope("admin:schreiben"));
    }
}
