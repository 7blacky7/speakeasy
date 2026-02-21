//! Ban-Service fuer Speakeasy
//!
//! Verwaltung von Benutzer- und IP-Bans. Unterstuetzt zeitlich begrenzte
//! und permanente Bans. Automatischer Cleanup abgelaufener Bans.

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use uuid::Uuid;

use speakeasy_db::{
    models::{BanRecord, NeuerBan},
    repository::BanRepository,
};

use crate::error::{AuthError, AuthResult};

/// Cleanup-Intervall fuer abgelaufene Bans: 1 Stunde
const BAN_CLEANUP_INTERVALL: Duration = Duration::from_secs(60 * 60);

/// Ban-Service – Verwaltung von Benutzer- und IP-Sperren
pub struct BanService<B: BanRepository> {
    ban_repo: Arc<B>,
}

impl<B: BanRepository + 'static> BanService<B> {
    /// Erstellt einen neuen BanService
    pub fn neu(ban_repo: Arc<B>) -> Arc<Self> {
        Arc::new(Self { ban_repo })
    }

    /// Startet den automatischen Cleanup-Task fuer abgelaufene Bans.
    ///
    /// Muss nach `neu()` aufgerufen werden. Der Task laeuft als unabhaengiger
    /// tokio-Task und bereinigt abgelaufene Bans im konfigurierten Intervall.
    /// Hinweis: Erfordert dass der BanService in einem tokio LocalSet-Kontext
    /// verwendet wird, da async_fn_in_trait keine Send-Garantie bietet.
    pub fn cleanup_task_starten(service: Arc<Self>) {
        // Starte einen separaten Task der periodisch cleanup_expired aufruft.
        // Wir nutzen spawn_local um die fehlende Send-Garantie zu umgehen.
        let handle = tokio::runtime::Handle::current();
        std::thread::spawn(move || {
            handle.block_on(async move {
                loop {
                    tokio::time::sleep(BAN_CLEANUP_INTERVALL).await;
                    match service.ban_repo.cleanup_expired().await {
                        Ok(anzahl) if anzahl > 0 => {
                            tracing::info!(anzahl, "Abgelaufene Bans bereinigt");
                        }
                        Ok(_) => {}
                        Err(e) => {
                            tracing::error!("Fehler beim Ban-Cleanup: {}", e);
                        }
                    }
                }
            });
        });
    }

    /// Bannt einen Benutzer
    ///
    /// `duration` – optionale Dauer; `None` = permanenter Ban
    pub async fn benutzer_bannen(
        &self,
        actor_id: Option<Uuid>,
        target_id: Uuid,
        grund: &str,
        dauer: Option<Duration>,
    ) -> AuthResult<BanRecord> {
        let laeuft_ab_am = dauer.map(|d| {
            Utc::now() + chrono::Duration::seconds(d.as_secs() as i64)
        });

        let ban = self
            .ban_repo
            .create(NeuerBan {
                user_id: Some(target_id),
                ip: None,
                reason: grund,
                banned_by: actor_id,
                expires_at: laeuft_ab_am,
            })
            .await?;

        tracing::info!(
            actor_id = ?actor_id,
            target_id = %target_id,
            ban_id = %ban.id,
            permanent = laeuft_ab_am.is_none(),
            "Benutzer gebannt"
        );

        Ok(ban)
    }

    /// Bannt eine IP-Adresse
    ///
    /// `duration` – optionale Dauer; `None` = permanenter Ban
    pub async fn ip_bannen(
        &self,
        actor_id: Option<Uuid>,
        ip: &str,
        grund: &str,
        dauer: Option<Duration>,
    ) -> AuthResult<BanRecord> {
        let laeuft_ab_am = dauer.map(|d| {
            Utc::now() + chrono::Duration::seconds(d.as_secs() as i64)
        });

        let ban = self
            .ban_repo
            .create(NeuerBan {
                user_id: None,
                ip: Some(ip),
                reason: grund,
                banned_by: actor_id,
                expires_at: laeuft_ab_am,
            })
            .await?;

        tracing::info!(
            actor_id = ?actor_id,
            ip = %ip,
            ban_id = %ban.id,
            permanent = laeuft_ab_am.is_none(),
            "IP gebannt"
        );

        Ok(ban)
    }

    /// Hebt einen Ban auf
    pub async fn ban_aufheben(&self, ban_id: Uuid) -> AuthResult<()> {
        let entfernt = self.ban_repo.remove(ban_id).await?;
        if entfernt {
            tracing::info!(ban_id = %ban_id, "Ban aufgehoben");
            Ok(())
        } else {
            Err(AuthError::intern(format!("Ban nicht gefunden: {ban_id}")))
        }
    }

    /// Prueft ob ein Benutzer oder eine IP aktuell gebannt ist
    ///
    /// Gibt `true` zurueck wenn ein aktiver Ban existiert.
    pub async fn ist_gebannt(
        &self,
        user_id: Option<Uuid>,
        ip: Option<&str>,
    ) -> AuthResult<bool> {
        Ok(self.ban_repo.is_banned(user_id, ip).await?.is_some())
    }

    /// Prueft und gibt einen AuthError zurueck wenn gebannt
    ///
    /// Nuetzlich beim Login um Bans direkt als Fehler zu behandeln.
    pub async fn ban_pruefen(
        &self,
        user_id: Option<Uuid>,
        ip: Option<&str>,
    ) -> AuthResult<()> {
        match self.ban_repo.is_banned(user_id, ip).await? {
            None => Ok(()),
            Some(ban) => {
                if ban.user_id.is_some() {
                    Err(AuthError::BenutzerGebannt(ban.reason))
                } else {
                    Err(AuthError::IpGebannt(ban.ip.unwrap_or_default()))
                }
            }
        }
    }

    /// Gibt alle aktiven Bans zurueck
    pub async fn aktive_bans_listen(&self) -> AuthResult<Vec<BanRecord>> {
        Ok(self.ban_repo.list(true).await?)
    }

    /// Gibt alle Bans (aktive + abgelaufene) zurueck
    pub async fn alle_bans_listen(&self) -> AuthResult<Vec<BanRecord>> {
        Ok(self.ban_repo.list(false).await?)
    }

    /// Laedt einen einzelnen Ban anhand seiner ID
    pub async fn ban_laden(&self, ban_id: Uuid) -> AuthResult<Option<BanRecord>> {
        Ok(self.ban_repo.get(ban_id).await?)
    }

    /// Bereinigt abgelaufene Bans manuell
    pub async fn abgelaufene_bereinigen(&self) -> AuthResult<u64> {
        Ok(self.ban_repo.cleanup_expired().await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use speakeasy_db::repository::DbResult;

    #[derive(Default)]
    struct TestBanRepo {
        bans: Mutex<Vec<BanRecord>>,
    }

    impl BanRepository for TestBanRepo {
        async fn create(&self, data: NeuerBan<'_>) -> DbResult<BanRecord> {
            let ban = BanRecord {
                id: Uuid::new_v4(),
                user_id: data.user_id,
                ip: data.ip.map(String::from),
                reason: data.reason.to_string(),
                banned_by: data.banned_by,
                expires_at: data.expires_at,
                created_at: Utc::now(),
            };
            self.bans.lock().unwrap().push(ban.clone());
            Ok(ban)
        }

        async fn get(&self, id: Uuid) -> DbResult<Option<BanRecord>> {
            Ok(self.bans.lock().unwrap().iter().find(|b| b.id == id).cloned())
        }

        async fn list(&self, nur_aktive: bool) -> DbResult<Vec<BanRecord>> {
            let bans = self.bans.lock().unwrap();
            let jetzt = Utc::now();
            Ok(bans.iter().filter(|b| {
                if nur_aktive {
                    b.expires_at.is_none_or(|e| e > jetzt)
                } else {
                    true
                }
            }).cloned().collect())
        }

        async fn remove(&self, id: Uuid) -> DbResult<bool> {
            let mut bans = self.bans.lock().unwrap();
            let vorher = bans.len();
            bans.retain(|b| b.id != id);
            Ok(bans.len() < vorher)
        }

        async fn is_banned(
            &self,
            user_id: Option<Uuid>,
            ip: Option<&str>,
        ) -> DbResult<Option<BanRecord>> {
            let bans = self.bans.lock().unwrap();
            let jetzt = Utc::now();
            Ok(bans.iter().find(|b| {
                let noch_aktiv = b.expires_at.is_none_or(|e| e > jetzt);
                if !noch_aktiv {
                    return false;
                }
                let user_match = user_id.is_some_and(|uid| b.user_id == Some(uid));
                let ip_match = ip.is_some_and(|i| b.ip.as_deref() == Some(i));
                user_match || ip_match
            }).cloned())
        }

        async fn cleanup_expired(&self) -> DbResult<u64> {
            let mut bans = self.bans.lock().unwrap();
            let jetzt = Utc::now();
            let vorher = bans.len();
            bans.retain(|b| b.expires_at.is_none_or(|e| e > jetzt));
            Ok((vorher - bans.len()) as u64)
        }
    }

    fn test_service() -> Arc<BanService<TestBanRepo>> {
        BanService::neu(Arc::new(TestBanRepo::default()))
    }

    #[tokio::test]
    async fn benutzer_bannen_und_pruefen() {
        let service = test_service();
        let target_id = Uuid::new_v4();

        service
            .benutzer_bannen(None, target_id, "Spam", None)
            .await
            .unwrap();

        assert!(service.ist_gebannt(Some(target_id), None).await.unwrap());
    }

    #[tokio::test]
    async fn ip_bannen_und_pruefen() {
        let service = test_service();

        service
            .ip_bannen(None, "192.168.1.100", "Angriff", None)
            .await
            .unwrap();

        assert!(service.ist_gebannt(None, Some("192.168.1.100")).await.unwrap());
        assert!(!service.ist_gebannt(None, Some("10.0.0.1")).await.unwrap());
    }

    #[tokio::test]
    async fn ban_aufheben() {
        let service = test_service();
        let target_id = Uuid::new_v4();

        let ban = service
            .benutzer_bannen(None, target_id, "Test", None)
            .await
            .unwrap();

        service.ban_aufheben(ban.id).await.unwrap();
        assert!(!service.ist_gebannt(Some(target_id), None).await.unwrap());
    }

    #[tokio::test]
    async fn zeitlich_begrenzter_ban_laeuft_ab() {
        let target_id = Uuid::new_v4();

        // Ban der bereits abgelaufen ist (negative Dauer simulieren via direkte Repo-Nutzung)
        let repo = Arc::new(TestBanRepo::default());
        let abgelaufener_ban = BanRecord {
            id: Uuid::new_v4(),
            user_id: Some(target_id),
            ip: None,
            reason: "Abgelaufen".into(),
            banned_by: None,
            expires_at: Some(Utc::now() - chrono::Duration::seconds(1)),
            created_at: Utc::now() - chrono::Duration::seconds(100),
        };
        repo.bans.lock().unwrap().push(abgelaufener_ban);

        let service2 = BanService::neu(repo);
        assert!(!service2.ist_gebannt(Some(target_id), None).await.unwrap());
    }
}
