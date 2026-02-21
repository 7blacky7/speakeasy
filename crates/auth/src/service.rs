//! Auth-Service fuer Speakeasy
//!
//! Zentraler Service fuer Registrierung, Login, Logout und Session-Verwaltung.
//! Nutzt die DB-Repositories und den Session-/API-Token-Store.

use std::sync::Arc;

use chrono::Utc;
use uuid::Uuid;

use speakeasy_db::{
    models::{BenutzerRecord, BenutzerUpdate, NeuerBenutzer},
    repository::UserRepository,
};

use crate::{
    api_token::{ApiTokenRecord, ApiTokenStore, ErstellterApiToken, NeuesApiToken},
    error::{AuthError, AuthResult},
    password::{passwort_hashen, passwort_verifizieren},
    session::{Session, SessionStore},
};

/// Auth-Service – zentraler Einstiegspunkt fuer alle Authentifizierungsvorgaenge
pub struct AuthService<U: UserRepository> {
    user_repo: Arc<U>,
    session_store: Arc<SessionStore>,
    api_token_store: Arc<ApiTokenStore>,
}

impl<U: UserRepository> AuthService<U> {
    /// Erstellt einen neuen AuthService
    pub fn neu(
        user_repo: Arc<U>,
        session_store: Arc<SessionStore>,
        api_token_store: Arc<ApiTokenStore>,
    ) -> Self {
        Self {
            user_repo,
            session_store,
            api_token_store,
        }
    }

    /// Registriert einen neuen Benutzer
    ///
    /// Prueft ob der Benutzername bereits vergeben ist und erstellt den Account.
    pub async fn registrieren(
        &self,
        username: &str,
        passwort: &str,
    ) -> AuthResult<BenutzerRecord> {
        // Pruefen ob Username bereits vergeben
        if let Some(_) = self.user_repo.get_by_name(username).await? {
            return Err(AuthError::BenutzernameVergeben(username.to_string()));
        }

        let passwort_hash = passwort_hashen(passwort)?;

        let benutzer = self
            .user_repo
            .create(NeuerBenutzer {
                username,
                password_hash: &passwort_hash,
            })
            .await?;

        tracing::info!(
            user_id = %benutzer.id,
            username = %benutzer.username,
            "Neuer Benutzer registriert"
        );

        Ok(benutzer)
    }

    /// Meldet einen Benutzer an und erstellt eine neue Session
    ///
    /// Gibt den Benutzer-Record und den Session-Token zurueck.
    pub async fn anmelden(
        &self,
        username: &str,
        passwort: &str,
    ) -> AuthResult<(BenutzerRecord, Session)> {
        // Benutzer laden
        let benutzer = self
            .user_repo
            .get_by_name(username)
            .await?
            .ok_or(AuthError::UngueltigeAnmeldedaten)?;

        // Benutzer aktiv?
        if !benutzer.is_active {
            return Err(AuthError::BenutzerGesperrt);
        }

        // Passwort pruefen
        let korrekt = passwort_verifizieren(passwort, &benutzer.password_hash)?;
        if !korrekt {
            tracing::warn!(username = %username, "Fehlgeschlagener Login-Versuch");
            return Err(AuthError::UngueltigeAnmeldedaten);
        }

        // Letzten Login aktualisieren
        self.user_repo.update_last_login(benutzer.id).await?;

        // Session erstellen
        let session = self.session_store.erstellen(benutzer.id).await?;

        tracing::info!(
            user_id = %benutzer.id,
            username = %benutzer.username,
            "Benutzer angemeldet"
        );

        Ok((benutzer, session))
    }

    /// Meldet einen Benutzer ab und invalidiert die Session
    pub async fn abmelden(&self, session_token: &str) -> AuthResult<()> {
        self.session_store.invalidieren(session_token).await?;
        tracing::debug!("Session invalidiert (Abmeldung)");
        Ok(())
    }

    /// Validiert einen Session-Token und gibt den zugehoerigen Benutzer zurueck
    pub async fn session_validieren(&self, token: &str) -> AuthResult<BenutzerRecord> {
        let session = self.session_store.validieren(token).await?;

        let benutzer = self
            .user_repo
            .get_by_id(session.user_id)
            .await?
            .ok_or_else(|| AuthError::BenutzerNichtGefunden(session.user_id.to_string()))?;

        if !benutzer.is_active {
            // Session invalidieren wenn Benutzer gesperrt wurde
            let _ = self.session_store.invalidieren(token).await;
            return Err(AuthError::BenutzerGesperrt);
        }

        Ok(benutzer)
    }

    /// Validiert einen API-Token und gibt Benutzer + Scopes zurueck
    pub async fn api_token_validieren(
        &self,
        token: &str,
    ) -> AuthResult<(BenutzerRecord, Vec<String>)> {
        let record = self.api_token_store.validieren(token).await?;

        let benutzer = self
            .user_repo
            .get_by_id(record.user_id)
            .await?
            .ok_or_else(|| AuthError::BenutzerNichtGefunden(record.user_id.to_string()))?;

        if !benutzer.is_active {
            return Err(AuthError::BenutzerGesperrt);
        }

        Ok((benutzer, record.scopes))
    }

    /// Aendert das Passwort eines Benutzers
    ///
    /// Erfordert das alte Passwort zur Verifikation.
    /// Invalidiert alle bestehenden Sessions des Benutzers.
    pub async fn passwort_aendern(
        &self,
        user_id: Uuid,
        altes_passwort: &str,
        neues_passwort: &str,
    ) -> AuthResult<()> {
        let benutzer = self
            .user_repo
            .get_by_id(user_id)
            .await?
            .ok_or_else(|| AuthError::BenutzerNichtGefunden(user_id.to_string()))?;

        // Altes Passwort pruefen
        let korrekt = passwort_verifizieren(altes_passwort, &benutzer.password_hash)?;
        if !korrekt {
            return Err(AuthError::UngueltigeAnmeldedaten);
        }

        // Neues Passwort hashen und speichern
        let neuer_hash = passwort_hashen(neues_passwort)?;
        self.user_repo
            .update(
                user_id,
                BenutzerUpdate {
                    password_hash: Some(neuer_hash),
                    ..Default::default()
                },
            )
            .await?;

        // Alle Sessions invalidieren (Sicherheit)
        let anzahl = self.session_store.alle_invalidieren(user_id).await;
        tracing::info!(
            user_id = %user_id,
            invalidierte_sessions = anzahl,
            "Passwort geaendert, Sessions invalidiert"
        );

        Ok(())
    }

    /// Erstellt einen neuen API-Token fuer einen Benutzer
    pub async fn api_token_erstellen(
        &self,
        user_id: Uuid,
        beschreibung: String,
        scopes: Vec<String>,
        laeuft_ab_am: Option<chrono::DateTime<Utc>>,
    ) -> AuthResult<ErstellterApiToken> {
        // Pruefen ob Benutzer existiert
        self.user_repo
            .get_by_id(user_id)
            .await?
            .ok_or_else(|| AuthError::BenutzerNichtGefunden(user_id.to_string()))?;

        let token = self
            .api_token_store
            .erstellen(NeuesApiToken {
                user_id,
                beschreibung,
                scopes,
                laeuft_ab_am,
            })
            .await?;

        tracing::info!(
            user_id = %user_id,
            token_praefix = %token.record.token_praefix,
            "Neuer API-Token erstellt"
        );

        Ok(token)
    }

    /// Widerruft einen API-Token
    pub async fn api_token_widerrufen(&self, token_id: Uuid) -> AuthResult<()> {
        self.api_token_store.widerrufen(token_id).await
    }

    /// Gibt alle API-Tokens eines Benutzers zurueck
    pub async fn api_tokens_fuer_user(&self, user_id: Uuid) -> Vec<ApiTokenRecord> {
        self.api_token_store.liste_fuer_user(user_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use speakeasy_db::DbError;

    // Minimaler In-Memory UserRepository fuer Tests
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

        async fn update(&self, id: Uuid, data: BenutzerUpdate) -> speakeasy_db::DbResult<BenutzerRecord> {
            let mut benutzer = self.benutzer.lock().unwrap();
            let user = benutzer.iter_mut().find(|u| u.id == id)
                .ok_or_else(|| DbError::nicht_gefunden(id.to_string()))?;
            if let Some(hash) = data.password_hash {
                user.password_hash = hash;
            }
            if let Some(aktiv) = data.is_active {
                user.is_active = aktiv;
            }
            Ok(user.clone())
        }

        async fn delete(&self, id: Uuid) -> speakeasy_db::DbResult<bool> {
            let mut benutzer = self.benutzer.lock().unwrap();
            let vorher = benutzer.len();
            benutzer.retain(|u| u.id != id);
            Ok(benutzer.len() < vorher)
        }

        async fn list(&self, nur_aktive: bool) -> speakeasy_db::DbResult<Vec<BenutzerRecord>> {
            let benutzer = self.benutzer.lock().unwrap();
            Ok(benutzer.iter().filter(|u| !nur_aktive || u.is_active).cloned().collect())
        }

        async fn authenticate(&self, username: &str, password_hash: &str) -> speakeasy_db::DbResult<Option<BenutzerRecord>> {
            Ok(self.benutzer.lock().unwrap().iter()
                .find(|u| u.username == username && u.password_hash == password_hash)
                .cloned())
        }

        async fn update_last_login(&self, id: Uuid) -> speakeasy_db::DbResult<()> {
            let mut benutzer = self.benutzer.lock().unwrap();
            if let Some(user) = benutzer.iter_mut().find(|u| u.id == id) {
                user.last_login = Some(Utc::now());
            }
            Ok(())
        }
    }

    fn test_service() -> AuthService<TestUserRepo> {
        let repo = Arc::new(TestUserRepo::default());
        let sessions = SessionStore::neu();
        let tokens = ApiTokenStore::neu();
        AuthService::neu(repo, sessions, tokens)
    }

    #[tokio::test]
    async fn registrieren_und_anmelden() {
        let service = test_service();

        let user = service
            .registrieren("testuser", "sicheres_passwort!")
            .await
            .expect("Registrierung fehlgeschlagen");

        assert_eq!(user.username, "testuser");
        assert!(user.is_active);

        let (angemeldeter, session) = service
            .anmelden("testuser", "sicheres_passwort!")
            .await
            .expect("Anmeldung fehlgeschlagen");

        assert_eq!(angemeldeter.id, user.id);
        assert!(!session.token.is_empty());
    }

    #[tokio::test]
    async fn doppelte_registrierung_schlaegt_fehl() {
        let service = test_service();
        service.registrieren("duplikat", "passwort").await.unwrap();
        let ergebnis = service.registrieren("duplikat", "anderes").await;
        assert!(matches!(ergebnis, Err(AuthError::BenutzernameVergeben(_))));
    }

    #[tokio::test]
    async fn falsches_passwort_abgelehnt() {
        let service = test_service();
        service.registrieren("user", "richtig").await.unwrap();
        let ergebnis = service.anmelden("user", "falsch").await;
        assert!(matches!(ergebnis, Err(AuthError::UngueltigeAnmeldedaten)));
    }

    #[tokio::test]
    async fn session_validierung() {
        let service = test_service();
        service.registrieren("sessionuser", "passwort").await.unwrap();
        let (_, session) = service.anmelden("sessionuser", "passwort").await.unwrap();

        let validierter = service.session_validieren(&session.token).await.unwrap();
        assert_eq!(validierter.username, "sessionuser");
    }

    #[tokio::test]
    async fn abmelden_invalidiert_session() {
        let service = test_service();
        service.registrieren("logoutuser", "passwort").await.unwrap();
        let (_, session) = service.anmelden("logoutuser", "passwort").await.unwrap();

        service.abmelden(&session.token).await.unwrap();
        let ergebnis = service.session_validieren(&session.token).await;
        assert!(matches!(ergebnis, Err(AuthError::SessionUngueltig)));
    }

    #[tokio::test]
    async fn passwort_aendern() {
        let service = test_service();
        let user = service.registrieren("pwuser", "altes_pw").await.unwrap();

        service
            .passwort_aendern(user.id, "altes_pw", "neues_pw")
            .await
            .unwrap();

        // Altes Passwort funktioniert nicht mehr
        let ergebnis = service.anmelden("pwuser", "altes_pw").await;
        assert!(matches!(ergebnis, Err(AuthError::UngueltigeAnmeldedaten)));

        // Neues Passwort funktioniert
        let (_, _) = service.anmelden("pwuser", "neues_pw").await.unwrap();
    }
}
