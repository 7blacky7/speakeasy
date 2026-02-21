//! API-Token-Management fuer Speakeasy
//!
//! Langlebige Tokens fuer Bots und den Commander-Client.
//! Tokens haben Scopes die bestimmte Aktionen erlauben.
//! Tokens werden in der Datenbank persistiert.

use chrono::{DateTime, Utc};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{AuthError, AuthResult};

/// Scope-Definitionen fuer API-Tokens
pub mod scopes {
    pub const CMD_CLIENTKICK: &str = "cmd:clientkick";
    pub const CMD_PERMISSIONWRITE: &str = "cmd:permissionwrite";
    pub const CMD_CHANNELCREATE: &str = "cmd:channelcreate";
    pub const CMD_CHANNELDELETE: &str = "cmd:channeldelete";
    pub const CMD_SERVERGROUPADD: &str = "cmd:servergroupadd";
    pub const CMD_SERVERGROUPREMOVE: &str = "cmd:servergroupremove";
    pub const CMD_BANLIST: &str = "cmd:banlist";
    pub const CMD_BAN: &str = "cmd:ban";
    pub const CMD_UNBAN: &str = "cmd:unban";
    pub const SERVER_INFO: &str = "server:info";
    pub const SERVER_EDIT: &str = "server:edit";
}

/// Ein API-Token-Eintrag (wie er in der DB gespeichert wird)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiTokenRecord {
    pub id: Uuid,
    /// ID des Benutzers dem dieses Token gehoert
    pub user_id: Uuid,
    /// Beschreibung des Tokens (z.B. "Mein Bot")
    pub beschreibung: String,
    /// Erlaubte Scopes
    pub scopes: Vec<String>,
    /// Gehashter Token-Wert (Argon2id)
    pub token_hash: String,
    /// Praefix des Tokens zur Anzeige (erste 8 Zeichen)
    pub token_praefix: String,
    /// Erstellungszeitpunkt
    pub erstellt_am: DateTime<Utc>,
    /// Ablaufzeitpunkt (None = nie)
    pub laeuft_ab_am: Option<DateTime<Utc>>,
    /// Ob das Token widerrufen wurde
    pub widerrufen: bool,
}

impl ApiTokenRecord {
    /// Gibt `true` zurueck wenn das Token noch gueltig ist
    pub fn ist_gueltig(&self) -> bool {
        if self.widerrufen {
            return false;
        }
        match self.laeuft_ab_am {
            None => true,
            Some(ablauf) => Utc::now() < ablauf,
        }
    }

    /// Prueft ob das Token einen bestimmten Scope hat
    pub fn hat_scope(&self, scope: &str) -> bool {
        self.scopes.iter().any(|s| s == scope)
    }
}

/// Eingabe zum Erstellen eines neuen API-Tokens
#[derive(Debug, Clone)]
pub struct NeuesApiToken {
    pub user_id: Uuid,
    pub beschreibung: String,
    pub scopes: Vec<String>,
    pub laeuft_ab_am: Option<DateTime<Utc>>,
}

/// Ergebnis der Token-Erstellung (Token-Wert nur einmal sichtbar!)
#[derive(Debug)]
pub struct ErstellterApiToken {
    pub record: ApiTokenRecord,
    /// Der eigentliche Token-Wert (nur bei Erstellung zurueckgegeben!)
    pub token_wert: String,
}

/// In-Memory API-Token-Store (ergaenzend zur DB-Persistenz)
///
/// Haltet gecachte Token-Records fuer schnelle Validierung.
/// Bei Server-Neustart werden Tokens aus der DB neu geladen.
#[derive(Debug, Default)]
pub struct ApiTokenStore {
    /// token_hash -> ApiTokenRecord (gecacht aus DB)
    tokens: tokio::sync::RwLock<Vec<ApiTokenRecord>>,
}

impl ApiTokenStore {
    pub fn neu() -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self::default())
    }

    /// Erstellt einen neuen API-Token
    ///
    /// Gibt den Token-Record und den Klartextwert zurueck.
    /// Der Klartextwert wird NUR einmal zurueckgegeben und nicht gespeichert!
    pub async fn erstellen(
        &self,
        eingabe: NeuesApiToken,
    ) -> AuthResult<ErstellterApiToken> {
        let token_wert = api_token_generieren();
        let token_hash = crate::password::passwort_hashen(&token_wert)?;
        let token_praefix = token_wert.chars().take(8).collect::<String>();

        let record = ApiTokenRecord {
            id: Uuid::new_v4(),
            user_id: eingabe.user_id,
            beschreibung: eingabe.beschreibung,
            scopes: eingabe.scopes,
            token_hash,
            token_praefix,
            erstellt_am: Utc::now(),
            laeuft_ab_am: eingabe.laeuft_ab_am,
            widerrufen: false,
        };

        self.tokens.write().await.push(record.clone());

        Ok(ErstellterApiToken {
            record,
            token_wert,
        })
    }

    /// Validiert einen API-Token-Wert
    ///
    /// Prueft gegen alle gecachten Tokens (Argon2id-Verifikation).
    /// Gibt den zugehoerigen Record zurueck wenn gueltig.
    pub async fn validieren(&self, token_wert: &str) -> AuthResult<ApiTokenRecord> {
        let tokens = self.tokens.read().await;

        for record in tokens.iter() {
            if !record.ist_gueltig() {
                continue;
            }
            match crate::password::passwort_verifizieren(token_wert, &record.token_hash) {
                Ok(true) => return Ok(record.clone()),
                Ok(false) => continue,
                Err(e) => {
                    tracing::warn!("Fehler bei Token-Verifikation: {}", e);
                    continue;
                }
            }
        }

        Err(AuthError::TokenUngueltig)
    }

    /// Widerruft einen API-Token anhand seiner ID
    pub async fn widerrufen(&self, token_id: Uuid) -> AuthResult<()> {
        let mut tokens = self.tokens.write().await;
        match tokens.iter_mut().find(|t| t.id == token_id) {
            None => Err(AuthError::TokenUngueltig),
            Some(token) => {
                token.widerrufen = true;
                tracing::info!(token_id = %token_id, "API-Token widerrufen");
                Ok(())
            }
        }
    }

    /// Gibt alle Tokens eines Benutzers zurueck
    pub async fn liste_fuer_user(&self, user_id: Uuid) -> Vec<ApiTokenRecord> {
        let tokens = self.tokens.read().await;
        tokens.iter().filter(|t| t.user_id == user_id).cloned().collect()
    }

    /// Laedt Token-Records in den Cache (beim Server-Start aus DB)
    pub async fn laden(&self, records: Vec<ApiTokenRecord>) {
        let mut tokens = self.tokens.write().await;
        *tokens = records;
        tracing::info!(anzahl = tokens.len(), "API-Tokens in Cache geladen");
    }

    /// Bereinigt abgelaufene (nicht widerrufene) Tokens aus dem Cache
    pub async fn cleanup_abgelaufene(&self) -> usize {
        let jetzt = Utc::now();
        let mut tokens = self.tokens.write().await;
        let vorher = tokens.len();
        tokens.retain(|t| {
            t.widerrufen || t.laeuft_ab_am.map_or(true, |ablauf| ablauf > jetzt)
        });
        vorher - tokens.len()
    }
}

/// Generiert einen kryptografisch sicheren API-Token
///
/// Format: "sk_" + 43 Zeichen URL-sicheres Base64 (256 Bit Entropie)
fn api_token_generieren() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    let encoded = base64::Engine::encode(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
        bytes,
    );
    format!("sk_{}", encoded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn token_erstellen_und_validieren() {
        let store = ApiTokenStore::neu();

        let erstellt = store
            .erstellen(NeuesApiToken {
                user_id: Uuid::new_v4(),
                beschreibung: "Test-Bot".into(),
                scopes: vec![scopes::SERVER_INFO.into()],
                laeuft_ab_am: None,
            })
            .await
            .expect("Token-Erstellung fehlgeschlagen");

        assert!(erstellt.token_wert.starts_with("sk_"));
        assert_eq!(erstellt.record.beschreibung, "Test-Bot");

        let validiert = store
            .validieren(&erstellt.token_wert)
            .await
            .expect("Token-Validierung fehlgeschlagen");

        assert_eq!(validiert.id, erstellt.record.id);
        assert!(validiert.hat_scope(scopes::SERVER_INFO));
        assert!(!validiert.hat_scope(scopes::CMD_BAN));
    }

    #[tokio::test]
    async fn ungueltige_token_abgelehnt() {
        let store = ApiTokenStore::neu();
        let ergebnis = store.validieren("sk_ungueltig").await;
        assert!(matches!(ergebnis, Err(AuthError::TokenUngueltig)));
    }

    #[tokio::test]
    async fn widerrufene_token_abgelehnt() {
        let store = ApiTokenStore::neu();
        let user_id = Uuid::new_v4();

        let erstellt = store
            .erstellen(NeuesApiToken {
                user_id,
                beschreibung: "Zu widerrufen".into(),
                scopes: vec![],
                laeuft_ab_am: None,
            })
            .await
            .unwrap();

        store.widerrufen(erstellt.record.id).await.unwrap();

        let ergebnis = store.validieren(&erstellt.token_wert).await;
        assert!(matches!(ergebnis, Err(AuthError::TokenUngueltig)));
    }

    #[test]
    fn record_ist_gueltig_pruefung() {
        let record = ApiTokenRecord {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            beschreibung: "Test".into(),
            scopes: vec![],
            token_hash: "hash".into(),
            token_praefix: "sk_12345".into(),
            erstellt_am: Utc::now(),
            laeuft_ab_am: None,
            widerrufen: false,
        };
        assert!(record.ist_gueltig());

        let widerrufener = ApiTokenRecord { widerrufen: true, ..record.clone() };
        assert!(!widerrufener.ist_gueltig());

        let abgelaufener = ApiTokenRecord {
            laeuft_ab_am: Some(Utc::now() - chrono::Duration::seconds(1)),
            ..record
        };
        assert!(!abgelaufener.ist_gueltig());
    }
}
