//! Permission-Service fuer Speakeasy
//!
//! Stellt einen caching-faehigen Service fuer Permission-Lookups bereit.
//! Nutzt die Permission-Engine aus crates/db und cached Ergebnisse
//! fuer haeufige Abfragen. Cache wird bei Aenderungen invalidiert.

use std::{collections::HashMap, sync::Arc};

use tokio::sync::RwLock;
use uuid::Uuid;

use speakeasy_db::{
    models::{BerechtigungsWert, EffektiveBerechtigung, TriState},
    repository::PermissionRepository,
};

use crate::error::{AuthError, AuthResult};

/// Cache-Key: (user_id, channel_id)
type CacheKey = (Uuid, Uuid);

/// Permission-Service mit optionalem Caching-Layer
pub struct PermissionService<P: PermissionRepository> {
    perm_repo: Arc<P>,
    /// Cache: (user_id, channel_id) -> HashMap<permission_key, EffektiveBerechtigung>
    cache: RwLock<HashMap<CacheKey, HashMap<String, EffektiveBerechtigung>>>,
}

impl<P: PermissionRepository> PermissionService<P> {
    /// Erstellt einen neuen PermissionService
    pub fn neu(perm_repo: Arc<P>) -> Arc<Self> {
        Arc::new(Self {
            perm_repo,
            cache: RwLock::new(HashMap::new()),
        })
    }

    /// Prueft ob ein Benutzer in einem Kanal eine bestimmte TriState-Berechtigung hat
    ///
    /// Wenn keine Permissions fuer den Benutzer existieren (leere Map),
    /// wird `true` zurueckgegeben (Default: erlaubt).
    /// Nur ein explizites Deny blockiert den Zugriff.
    /// Wenn Permissions vorhanden sind aber der Key fehlt, gilt ebenfalls erlaubt.
    pub async fn berechtigung_pruefen(
        &self,
        user_id: Uuid,
        channel_id: Uuid,
        permission_key: &str,
    ) -> AuthResult<bool> {
        let perms = self.alle_berechtigungen_laden(user_id, channel_id).await?;
        match perms.get(permission_key) {
            // Keine Regel fuer diesen Key -> erlaubt (wie TeamSpeak)
            None => Ok(true),
            Some(eb) => match &eb.wert {
                // Nur explizites Deny blockiert
                BerechtigungsWert::TriState(ts) => Ok(*ts != TriState::Deny),
                _ => Ok(true),
            },
        }
    }

    /// Prueft einen IntLimit-Berechtigungswert
    ///
    /// Gibt `None` zurueck wenn die Berechtigung nicht gesetzt ist.
    pub async fn int_berechtigung_pruefen(
        &self,
        user_id: Uuid,
        channel_id: Uuid,
        permission_key: &str,
    ) -> AuthResult<Option<i64>> {
        let perms = self.alle_berechtigungen_laden(user_id, channel_id).await?;
        match perms.get(permission_key) {
            None => Ok(None),
            Some(eb) => match &eb.wert {
                BerechtigungsWert::IntLimit(limit) => Ok(Some(*limit)),
                _ => Ok(None),
            },
        }
    }

    /// Gibt alle effektiven Berechtigungen eines Benutzers in einem Kanal zurueck
    pub async fn alle_berechtigungen_holen(
        &self,
        user_id: Uuid,
        channel_id: Uuid,
    ) -> AuthResult<HashMap<String, BerechtigungsWert>> {
        let perms = self.alle_berechtigungen_laden(user_id, channel_id).await?;
        Ok(perms.into_iter().map(|(k, eb)| (k, eb.wert)).collect())
    }

    /// Erfordert eine Berechtigung – gibt Fehler wenn nicht erlaubt
    ///
    /// Wirft `AuthError::ZugriffVerweigert` wenn die Berechtigung nicht gesetzt
    /// oder auf Deny gesetzt ist.
    pub async fn berechtigung_erfordern(
        &self,
        user_id: Uuid,
        channel_id: Uuid,
        permission_key: &str,
    ) -> AuthResult<()> {
        let hat_berechtigung = self
            .berechtigung_pruefen(user_id, channel_id, permission_key)
            .await?;

        if hat_berechtigung {
            Ok(())
        } else {
            Err(AuthError::ZugriffVerweigert(permission_key.to_string()))
        }
    }

    /// Invalidiert den Cache fuer einen bestimmten Benutzer/Kanal
    pub async fn cache_invalidieren(&self, user_id: Uuid, channel_id: Uuid) {
        let mut cache = self.cache.write().await;
        cache.remove(&(user_id, channel_id));
        tracing::debug!(
            user_id = %user_id,
            channel_id = %channel_id,
            "Permission-Cache invalidiert"
        );
    }

    /// Invalidiert den gesamten Cache (z.B. nach Gruppen-Aenderungen)
    pub async fn cache_komplett_invalidieren(&self) {
        let mut cache = self.cache.write().await;
        let anzahl = cache.len();
        cache.clear();
        tracing::info!(eintraege = anzahl, "Permission-Cache komplett invalidiert");
    }

    /// Gibt die Groesse des Caches zurueck
    pub async fn cache_groesse(&self) -> usize {
        self.cache.read().await.len()
    }

    // --- Interne Hilfsmethoden ---

    /// Laedt Berechtigungen (aus Cache oder DB)
    async fn alle_berechtigungen_laden(
        &self,
        user_id: Uuid,
        channel_id: Uuid,
    ) -> AuthResult<HashMap<String, EffektiveBerechtigung>> {
        let schluessel = (user_id, channel_id);

        // Cache-Treffer pruefen
        {
            let cache = self.cache.read().await;
            if let Some(perms) = cache.get(&schluessel) {
                return Ok(perms.clone());
            }
        }

        // Cache-Miss: aus DB laden
        let effektive = self
            .perm_repo
            .resolve_effective_permissions(user_id, channel_id)
            .await?;

        let perm_map: HashMap<String, EffektiveBerechtigung> = effektive
            .into_iter()
            .map(|eb| (eb.permission_key.clone(), eb))
            .collect();

        // In Cache speichern
        {
            let mut cache = self.cache.write().await;
            cache.insert(schluessel, perm_map.clone());
        }

        Ok(perm_map)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use speakeasy_db::{
        models::{BerechtigungsZiel, TriState},
        repository::DbResult,
    };

    struct TestPermRepo {
        /// Vordefinierte Berechtigungen: user_id -> channel_id -> key -> wert
        perms: HashMap<(Uuid, Uuid), Vec<EffektiveBerechtigung>>,
    }

    impl TestPermRepo {
        fn mit_grant(user_id: Uuid, channel_id: Uuid, key: &str) -> Self {
            let mut perms = HashMap::new();
            perms.insert(
                (user_id, channel_id),
                vec![EffektiveBerechtigung {
                    permission_key: key.to_string(),
                    wert: BerechtigungsWert::TriState(TriState::Grant),
                    quelle: "Test".to_string(),
                }],
            );
            Self { perms }
        }

        fn mit_deny(user_id: Uuid, channel_id: Uuid, key: &str) -> Self {
            let mut perms = HashMap::new();
            perms.insert(
                (user_id, channel_id),
                vec![EffektiveBerechtigung {
                    permission_key: key.to_string(),
                    wert: BerechtigungsWert::TriState(TriState::Deny),
                    quelle: "Test".to_string(),
                }],
            );
            Self { perms }
        }

        fn leer() -> Self {
            Self {
                perms: HashMap::new(),
            }
        }
    }

    impl PermissionRepository for TestPermRepo {
        async fn get_permissions(
            &self,
            _ziel: &BerechtigungsZiel,
            _channel_id: Option<Uuid>,
        ) -> DbResult<Vec<(String, BerechtigungsWert)>> {
            Ok(vec![])
        }

        async fn set_permission(
            &self,
            _ziel: &BerechtigungsZiel,
            _permission_key: &str,
            _wert: BerechtigungsWert,
            _channel_id: Option<Uuid>,
        ) -> DbResult<()> {
            Ok(())
        }

        async fn remove_permission(
            &self,
            _ziel: &BerechtigungsZiel,
            _permission_key: &str,
            _channel_id: Option<Uuid>,
        ) -> DbResult<bool> {
            Ok(false)
        }

        async fn resolve_effective_permissions(
            &self,
            user_id: Uuid,
            channel_id: Uuid,
        ) -> DbResult<Vec<EffektiveBerechtigung>> {
            Ok(self
                .perms
                .get(&(user_id, channel_id))
                .cloned()
                .unwrap_or_default())
        }
    }

    #[tokio::test]
    async fn berechtigung_vorhanden_gibt_true() {
        let user_id = Uuid::new_v4();
        let channel_id = Uuid::new_v4();
        let repo = Arc::new(TestPermRepo::mit_grant(user_id, channel_id, "can_speak"));
        let service = PermissionService::neu(repo);

        let ergebnis = service
            .berechtigung_pruefen(user_id, channel_id, "can_speak")
            .await
            .unwrap();
        assert!(ergebnis);
    }

    #[tokio::test]
    async fn fehlende_berechtigung_gibt_true_default_erlaubt() {
        let user_id = Uuid::new_v4();
        let channel_id = Uuid::new_v4();
        let repo = Arc::new(TestPermRepo::leer());
        let service = PermissionService::neu(repo);

        // Keine Permissions vorhanden -> Default: erlaubt
        let ergebnis = service
            .berechtigung_pruefen(user_id, channel_id, "can_speak")
            .await
            .unwrap();
        assert!(ergebnis);
    }

    #[tokio::test]
    async fn explizites_deny_blockiert() {
        let user_id = Uuid::new_v4();
        let channel_id = Uuid::new_v4();
        let repo = Arc::new(TestPermRepo::mit_deny(user_id, channel_id, "can_ban"));
        let service = PermissionService::neu(repo);

        let ergebnis = service
            .berechtigung_pruefen(user_id, channel_id, "can_ban")
            .await
            .unwrap();
        assert!(!ergebnis);
    }

    #[tokio::test]
    async fn berechtigung_erfordern_erlaubt_wenn_keine_regel() {
        let user_id = Uuid::new_v4();
        let channel_id = Uuid::new_v4();
        let repo = Arc::new(TestPermRepo::leer());
        let service = PermissionService::neu(repo);

        // Keine Permissions -> Default erlaubt -> kein Fehler
        let ergebnis = service
            .berechtigung_erfordern(user_id, channel_id, "can_ban")
            .await;
        assert!(ergebnis.is_ok());
    }

    #[tokio::test]
    async fn berechtigung_erfordern_wirft_fehler_bei_deny() {
        let user_id = Uuid::new_v4();
        let channel_id = Uuid::new_v4();
        let repo = Arc::new(TestPermRepo::mit_deny(user_id, channel_id, "can_ban"));
        let service = PermissionService::neu(repo);

        let ergebnis = service
            .berechtigung_erfordern(user_id, channel_id, "can_ban")
            .await;
        assert!(matches!(ergebnis, Err(AuthError::ZugriffVerweigert(_))));
    }

    #[tokio::test]
    async fn cache_wird_befuellt_und_invalidiert() {
        let user_id = Uuid::new_v4();
        let channel_id = Uuid::new_v4();
        let repo = Arc::new(TestPermRepo::mit_grant(user_id, channel_id, "can_speak"));
        let service = PermissionService::neu(repo);

        assert_eq!(service.cache_groesse().await, 0);

        service
            .berechtigung_pruefen(user_id, channel_id, "can_speak")
            .await
            .unwrap();
        assert_eq!(service.cache_groesse().await, 1);

        service.cache_invalidieren(user_id, channel_id).await;
        assert_eq!(service.cache_groesse().await, 0);
    }
}
