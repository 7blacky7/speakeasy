//! Invite-Service fuer Speakeasy
//!
//! Verwaltung von Einladungscodes. Unterstuetzt zeitlich begrenzte Einladungen
//! mit optionaler maximaler Nutzungsanzahl und automatischer Gruppen-Zuweisung.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use rand::RngCore;
use uuid::Uuid;

use speakeasy_db::{
    models::{EinladungRecord, NeueEinladung},
    repository::{InviteRepository, ServerGroupRepository, UserRepository},
    DbError,
};

use crate::error::{AuthError, AuthResult};

/// Laenge des generierten Einladungscodes (Zeichen)
const INVITE_CODE_LAENGE: usize = 8;

/// Invite-Service – Verwaltung von Einladungscodes
pub struct InviteService<I: InviteRepository, U: UserRepository, G: ServerGroupRepository> {
    invite_repo: Arc<I>,
    user_repo: Arc<U>,
    group_repo: Arc<G>,
}

impl<I: InviteRepository, U: UserRepository, G: ServerGroupRepository>
    InviteService<I, U, G>
{
    /// Erstellt einen neuen InviteService
    pub fn neu(invite_repo: Arc<I>, user_repo: Arc<U>, group_repo: Arc<G>) -> Arc<Self> {
        Arc::new(Self {
            invite_repo,
            user_repo,
            group_repo,
        })
    }

    /// Erstellt einen neuen Einladungscode
    ///
    /// - `channel_id`: optionaler Ziel-Kanal (fuer Kanal-spezifische Einladungen)
    /// - `group_id`: optionale Server-Gruppe die dem Benutzer zugewiesen wird
    /// - `max_uses`: maximale Nutzungsanzahl (0 = unbegrenzt)
    /// - `laeuft_ab_am`: optionales Ablaufdatum
    pub async fn einladung_erstellen(
        &self,
        ersteller_id: Uuid,
        channel_id: Option<Uuid>,
        group_id: Option<Uuid>,
        max_uses: i64,
        laeuft_ab_am: Option<DateTime<Utc>>,
    ) -> AuthResult<EinladungRecord> {
        // Pruefen ob Ersteller existiert
        self.user_repo
            .get_by_id(ersteller_id)
            .await?
            .ok_or_else(|| AuthError::BenutzerNichtGefunden(ersteller_id.to_string()))?;

        let code = invite_code_generieren();

        let einladung = self
            .invite_repo
            .create(NeueEinladung {
                code: &code,
                channel_id,
                assigned_group_id: group_id,
                max_uses,
                expires_at: laeuft_ab_am,
                created_by: ersteller_id,
            })
            .await?;

        tracing::info!(
            ersteller_id = %ersteller_id,
            code = %code,
            max_uses = max_uses,
            "Einladung erstellt"
        );

        Ok(einladung)
    }

    /// Verwendet einen Einladungscode
    ///
    /// Prueft Gueltigkeit, erhoet den Zaehler und weist optional eine Gruppe zu.
    pub async fn einladung_verwenden(
        &self,
        code: &str,
        user_id: Uuid,
    ) -> AuthResult<EinladungRecord> {
        // Einladung verwenden (DB prueft Gueltigkeit und erhoet Zaehler)
        let einladung = self
            .invite_repo
            .use_invite(code)
            .await
            .map_err(|e| match e {
                DbError::EinladungUngueltig => AuthError::EinladungUngueltig,
                DbError::EinladungErschoepft => AuthError::EinladungErschoepft,
                other => AuthError::Datenbank(other),
            })?
            .ok_or(AuthError::EinladungUngueltig)?;

        // Gruppe zuweisen wenn vorhanden
        if let Some(group_id) = einladung.assigned_group_id {
            match self.group_repo.add_member(group_id, user_id).await {
                Ok(()) => {
                    tracing::info!(
                        user_id = %user_id,
                        group_id = %group_id,
                        "Benutzer via Einladung zu Gruppe hinzugefuegt"
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        user_id = %user_id,
                        group_id = %group_id,
                        fehler = %e,
                        "Gruppen-Zuweisung via Einladung fehlgeschlagen"
                    );
                }
            }
        }

        tracing::info!(
            user_id = %user_id,
            code = %code,
            used_count = einladung.used_count,
            "Einladung verwendet"
        );

        Ok(einladung)
    }

    /// Widerruft einen Einladungscode
    pub async fn einladung_widerrufen(&self, einladung_id: Uuid) -> AuthResult<()> {
        let widerrufen = self.invite_repo.revoke(einladung_id).await?;
        if widerrufen {
            tracing::info!(einladung_id = %einladung_id, "Einladung widerrufen");
            Ok(())
        } else {
            Err(AuthError::EinladungUngueltig)
        }
    }

    /// Laedt eine Einladung anhand ihres Codes
    pub async fn einladung_laden(&self, code: &str) -> AuthResult<Option<EinladungRecord>> {
        Ok(self.invite_repo.get_by_code(code).await?)
    }

    /// Laedt eine Einladung anhand ihrer ID
    pub async fn einladung_laden_by_id(
        &self,
        einladung_id: Uuid,
    ) -> AuthResult<Option<EinladungRecord>> {
        Ok(self.invite_repo.get(einladung_id).await?)
    }

    /// Listet alle Einladungen eines Erstellers auf
    pub async fn einladungen_listen(
        &self,
        ersteller_id: Option<Uuid>,
    ) -> AuthResult<Vec<EinladungRecord>> {
        Ok(self.invite_repo.list(ersteller_id).await?)
    }
}

/// Generiert einen zufaelligen Einladungscode (alphanumerisch, Grossbuchstaben)
fn invite_code_generieren() -> String {
    const ZEICHEN: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut rng = rand::thread_rng();
    let mut bytes = vec![0u8; INVITE_CODE_LAENGE];
    rng.fill_bytes(&mut bytes);
    bytes
        .iter()
        .map(|b| ZEICHEN[(*b as usize) % ZEICHEN.len()] as char)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use speakeasy_db::{
        models::{BenutzerRecord, NeuerBenutzer, BenutzerUpdate, ServerGruppeRecord, NeueServerGruppe},
        repository::{DbResult, UserRepository, ServerGroupRepository},
    };

    #[derive(Default)]
    struct TestUserRepo {
        benutzer: Mutex<Vec<BenutzerRecord>>,
    }

    impl UserRepository for TestUserRepo {
        async fn create(&self, data: NeuerBenutzer<'_>) -> DbResult<BenutzerRecord> {
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
        async fn get_by_id(&self, id: Uuid) -> DbResult<Option<BenutzerRecord>> {
            Ok(self.benutzer.lock().unwrap().iter().find(|u| u.id == id).cloned())
        }
        async fn get_by_name(&self, username: &str) -> DbResult<Option<BenutzerRecord>> {
            Ok(self.benutzer.lock().unwrap().iter().find(|u| u.username == username).cloned())
        }
        async fn update(&self, id: Uuid, _data: BenutzerUpdate) -> DbResult<BenutzerRecord> {
            self.get_by_id(id).await?.ok_or_else(|| DbError::nicht_gefunden(id.to_string()))
        }
        async fn delete(&self, _id: Uuid) -> DbResult<bool> { Ok(false) }
        async fn list(&self, _nur_aktive: bool) -> DbResult<Vec<BenutzerRecord>> {
            Ok(self.benutzer.lock().unwrap().clone())
        }
        async fn authenticate(&self, _u: &str, _p: &str) -> DbResult<Option<BenutzerRecord>> {
            Ok(None)
        }
        async fn update_last_login(&self, _id: Uuid) -> DbResult<()> { Ok(()) }
    }

    #[derive(Default)]
    struct TestGroupRepo {
        mitglieder: Mutex<Vec<(Uuid, Uuid)>>, // (group_id, user_id)
    }

    impl ServerGroupRepository for TestGroupRepo {
        async fn create(&self, _data: NeueServerGruppe<'_>) -> DbResult<ServerGruppeRecord> {
            unimplemented!()
        }
        async fn get(&self, _id: Uuid) -> DbResult<Option<ServerGruppeRecord>> { Ok(None) }
        async fn list(&self) -> DbResult<Vec<ServerGruppeRecord>> { Ok(vec![]) }
        async fn list_for_user(&self, _user_id: Uuid) -> DbResult<Vec<ServerGruppeRecord>> {
            Ok(vec![])
        }
        async fn add_member(&self, group_id: Uuid, user_id: Uuid) -> DbResult<()> {
            self.mitglieder.lock().unwrap().push((group_id, user_id));
            Ok(())
        }
        async fn remove_member(&self, _group_id: Uuid, _user_id: Uuid) -> DbResult<bool> {
            Ok(false)
        }
        async fn get_default(&self) -> DbResult<Option<ServerGruppeRecord>> { Ok(None) }
        async fn delete(&self, _id: Uuid) -> DbResult<bool> { Ok(false) }
    }

    #[derive(Default)]
    struct TestInviteRepo {
        einladungen: Mutex<Vec<EinladungRecord>>,
    }

    impl InviteRepository for TestInviteRepo {
        async fn create(&self, data: NeueEinladung<'_>) -> DbResult<EinladungRecord> {
            let record = EinladungRecord {
                id: Uuid::new_v4(),
                code: data.code.to_string(),
                channel_id: data.channel_id,
                assigned_group_id: data.assigned_group_id,
                max_uses: data.max_uses,
                used_count: 0,
                expires_at: data.expires_at,
                created_by: data.created_by,
                created_at: Utc::now(),
            };
            self.einladungen.lock().unwrap().push(record.clone());
            Ok(record)
        }
        async fn get(&self, id: Uuid) -> DbResult<Option<EinladungRecord>> {
            Ok(self.einladungen.lock().unwrap().iter().find(|e| e.id == id).cloned())
        }
        async fn get_by_code(&self, code: &str) -> DbResult<Option<EinladungRecord>> {
            Ok(self.einladungen.lock().unwrap().iter().find(|e| e.code == code).cloned())
        }
        async fn list(&self, created_by: Option<Uuid>) -> DbResult<Vec<EinladungRecord>> {
            let einladungen = self.einladungen.lock().unwrap();
            Ok(einladungen.iter().filter(|e| {
                created_by.is_none_or(|id| e.created_by == id)
            }).cloned().collect())
        }
        async fn use_invite(&self, code: &str) -> DbResult<Option<EinladungRecord>> {
            let mut einladungen = self.einladungen.lock().unwrap();
            let jetzt = Utc::now();
            match einladungen.iter_mut().find(|e| e.code == code) {
                None => Ok(None),
                Some(e) => {
                    if e.expires_at.is_some_and(|a| a <= jetzt) {
                        return Err(DbError::EinladungUngueltig);
                    }
                    if e.max_uses > 0 && e.used_count >= e.max_uses {
                        return Err(DbError::EinladungErschoepft);
                    }
                    e.used_count += 1;
                    Ok(Some(e.clone()))
                }
            }
        }
        async fn revoke(&self, id: Uuid) -> DbResult<bool> {
            let mut einladungen = self.einladungen.lock().unwrap();
            let vorher = einladungen.len();
            einladungen.retain(|e| e.id != id);
            Ok(einladungen.len() < vorher)
        }
    }

    async fn test_setup() -> (
        Arc<InviteService<TestInviteRepo, TestUserRepo, TestGroupRepo>>,
        Uuid,
    ) {
        let user_repo = Arc::new(TestUserRepo::default());
        let ersteller = user_repo
            .create(NeuerBenutzer { username: "ersteller", password_hash: "hash" })
            .await
            .unwrap();

        let service = InviteService::neu(
            Arc::new(TestInviteRepo::default()),
            Arc::clone(&user_repo),
            Arc::new(TestGroupRepo::default()),
        );

        (service, ersteller.id)
    }

    #[tokio::test]
    async fn einladung_erstellen_und_verwenden() {
        let (service, ersteller_id) = test_setup().await;

        let einladung = service
            .einladung_erstellen(ersteller_id, None, None, 5, None)
            .await
            .unwrap();

        assert_eq!(einladung.max_uses, 5);
        assert_eq!(einladung.used_count, 0);
        assert_eq!(einladung.code.len(), INVITE_CODE_LAENGE);

        let user_id = Uuid::new_v4();
        let verwendet = service.einladung_verwenden(&einladung.code, user_id).await.unwrap();
        assert_eq!(verwendet.used_count, 1);
    }

    #[tokio::test]
    async fn ungueltige_einladung_abgelehnt() {
        let (service, _) = test_setup().await;
        let user_id = Uuid::new_v4();
        let ergebnis = service.einladung_verwenden("UNGUELTIG", user_id).await;
        assert!(matches!(ergebnis, Err(AuthError::EinladungUngueltig)));
    }

    #[tokio::test]
    async fn erschoepfte_einladung_abgelehnt() {
        let (service, ersteller_id) = test_setup().await;

        let einladung = service
            .einladung_erstellen(ersteller_id, None, None, 1, None)
            .await
            .unwrap();

        service.einladung_verwenden(&einladung.code, Uuid::new_v4()).await.unwrap();

        let ergebnis = service.einladung_verwenden(&einladung.code, Uuid::new_v4()).await;
        assert!(matches!(ergebnis, Err(AuthError::EinladungErschoepft)));
    }

    #[test]
    fn invite_code_format() {
        let code = invite_code_generieren();
        assert_eq!(code.len(), INVITE_CODE_LAENGE);
        assert!(code.chars().all(|c| c.is_ascii_alphanumeric() && !matches!(c, '0' | '1' | 'I' | 'O')));
    }
}
