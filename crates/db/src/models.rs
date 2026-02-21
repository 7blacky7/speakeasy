//! Datenbankmodelle fuer Speakeasy
//!
//! Diese Typen repraesentieren Datensaetze aus der Datenbank.
//! Sie sind von den Domain-Typen getrennt und dienen als reine Datenuebertragungsobjekte.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Benutzer
// ---------------------------------------------------------------------------

/// Benutzer-Datensatz aus der Datenbank
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenutzerRecord {
    pub id: Uuid,
    pub username: String,
    pub password_hash: String,
    pub created_at: DateTime<Utc>,
    pub last_login: Option<DateTime<Utc>>,
    pub is_active: bool,
}

/// Daten zum Erstellen eines neuen Benutzers
#[derive(Debug, Clone)]
pub struct NeuerBenutzer<'a> {
    pub username: &'a str,
    pub password_hash: &'a str,
}

/// Daten zum Aktualisieren eines Benutzers
#[derive(Debug, Clone, Default)]
pub struct BenutzerUpdate {
    pub username: Option<String>,
    pub password_hash: Option<String>,
    pub is_active: Option<bool>,
    pub last_login: Option<DateTime<Utc>>,
}

// ---------------------------------------------------------------------------
// Kanaele
// ---------------------------------------------------------------------------

/// Kanal-Typ
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KanalTyp {
    Voice,
    Text,
}

impl KanalTyp {
    pub fn als_str(&self) -> &'static str {
        match self {
            Self::Voice => "voice",
            Self::Text => "text",
        }
    }
}

impl std::str::FromStr for KanalTyp {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "voice" => Ok(Self::Voice),
            "text" => Ok(Self::Text),
            other => Err(format!("Unbekannter Kanal-Typ: {other}")),
        }
    }
}

/// Kanal-Datensatz aus der Datenbank
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KanalRecord {
    pub id: Uuid,
    pub name: String,
    pub parent_id: Option<Uuid>,
    pub topic: Option<String>,
    pub password_hash: Option<String>,
    pub max_clients: i64,
    pub is_default: bool,
    pub sort_order: i64,
    pub channel_type: KanalTyp,
    pub created_at: DateTime<Utc>,
}

/// Daten zum Erstellen eines neuen Kanals
#[derive(Debug, Clone)]
pub struct NeuerKanal<'a> {
    pub name: &'a str,
    pub parent_id: Option<Uuid>,
    pub topic: Option<&'a str>,
    pub password_hash: Option<&'a str>,
    pub max_clients: i64,
    pub is_default: bool,
    pub sort_order: i64,
    pub channel_type: KanalTyp,
}

impl Default for NeuerKanal<'_> {
    fn default() -> Self {
        Self {
            name: "",
            parent_id: None,
            topic: None,
            password_hash: None,
            max_clients: 0,
            is_default: false,
            sort_order: 0,
            channel_type: KanalTyp::Voice,
        }
    }
}

/// Daten zum Aktualisieren eines Kanals
#[derive(Debug, Clone, Default)]
pub struct KanalUpdate {
    pub name: Option<String>,
    pub parent_id: Option<Option<Uuid>>,
    pub topic: Option<Option<String>>,
    pub password_hash: Option<Option<String>>,
    pub max_clients: Option<i64>,
    pub is_default: Option<bool>,
    pub sort_order: Option<i64>,
}

// ---------------------------------------------------------------------------
// Server-Gruppen
// ---------------------------------------------------------------------------

/// Server-Gruppen-Datensatz
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerGruppeRecord {
    pub id: Uuid,
    pub name: String,
    pub priority: i64,
    pub is_default: bool,
    pub permissions: serde_json::Value,
}

/// Daten zum Erstellen einer neuen Server-Gruppe
#[derive(Debug, Clone)]
pub struct NeueServerGruppe<'a> {
    pub name: &'a str,
    pub priority: i64,
    pub is_default: bool,
}

// ---------------------------------------------------------------------------
// Kanal-Gruppen
// ---------------------------------------------------------------------------

/// Kanal-Gruppen-Datensatz
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KanalGruppeRecord {
    pub id: Uuid,
    pub name: String,
    pub permissions: serde_json::Value,
}

/// Daten zum Erstellen einer neuen Kanal-Gruppe
#[derive(Debug, Clone)]
pub struct NeueKanalGruppe<'a> {
    pub name: &'a str,
}

// ---------------------------------------------------------------------------
// Berechtigungen (Permission-System)
// ---------------------------------------------------------------------------

/// Wert einer Berechtigung
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum BerechtigungsWert {
    /// Dreiwertig: Grant/Deny/Skip
    TriState(TriState),
    /// Zahlenbegrenzung
    IntLimit(i64),
    /// Liste erlaubter Werte
    Scope(Vec<String>),
}

/// Dreiwertiger Zustand fuer Berechtigungen
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriState {
    Grant,
    Deny,
    Skip,
}

impl TriState {
    /// Konvertiert aus optionalem Integer (None=Skip, 1=Grant, 0=Deny)
    pub fn from_opt_int(v: Option<i64>) -> Self {
        match v {
            None => Self::Skip,
            Some(1) => Self::Grant,
            Some(_) => Self::Deny,
        }
    }

    /// Konvertiert zu optionalem Integer
    pub fn to_opt_int(self) -> Option<i64> {
        match self {
            Self::Skip => None,
            Self::Grant => Some(1),
            Self::Deny => Some(0),
        }
    }
}

/// Berechtigung-Datensatz
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BerechtigungRecord {
    pub id: Uuid,
    pub target_type: String,
    pub target_id: Option<Uuid>,
    pub permission_key: String,
    pub wert: BerechtigungsWert,
    pub channel_id: Option<Uuid>,
}

/// Ziel-Typ einer Berechtigung
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BerechtigungsZiel {
    Benutzer(Uuid),
    ServerGruppe(Uuid),
    KanalGruppe(Uuid),
    ServerDefault,
    KanalDefault(Uuid),
}

impl BerechtigungsZiel {
    pub fn typ_str(&self) -> &'static str {
        match self {
            Self::Benutzer(_) => "user",
            Self::ServerGruppe(_) => "server_group",
            Self::KanalGruppe(_) => "channel_group",
            Self::ServerDefault => "server_default",
            Self::KanalDefault(_) => "channel_default",
        }
    }

    pub fn id(&self) -> Option<Uuid> {
        match self {
            Self::Benutzer(id) => Some(*id),
            Self::ServerGruppe(id) => Some(*id),
            Self::KanalGruppe(id) => Some(*id),
            Self::ServerDefault => None,
            Self::KanalDefault(id) => Some(*id),
        }
    }
}

/// Aufgeloeste effektive Berechtigung
#[derive(Debug, Clone)]
pub struct EffektiveBerechtigung {
    pub permission_key: String,
    pub wert: BerechtigungsWert,
    /// Quelle der Berechtigung (zur Diagnose)
    pub quelle: String,
}

// ---------------------------------------------------------------------------
// Bans
// ---------------------------------------------------------------------------

/// Ban-Datensatz
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BanRecord {
    pub id: Uuid,
    pub user_id: Option<Uuid>,
    pub ip: Option<String>,
    pub reason: String,
    pub banned_by: Option<Uuid>,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// Daten zum Erstellen eines Bans
#[derive(Debug, Clone)]
pub struct NeuerBan<'a> {
    pub user_id: Option<Uuid>,
    pub ip: Option<&'a str>,
    pub reason: &'a str,
    pub banned_by: Option<Uuid>,
    pub expires_at: Option<DateTime<Utc>>,
}

// ---------------------------------------------------------------------------
// Audit-Log
// ---------------------------------------------------------------------------

/// Audit-Log-Eintrag
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogRecord {
    pub id: Uuid,
    pub actor_id: Option<Uuid>,
    pub action: String,
    pub target_type: Option<String>,
    pub target_id: Option<String>,
    pub details: serde_json::Value,
    pub timestamp: DateTime<Utc>,
}

/// Filter fuer Audit-Log-Abfragen
#[derive(Debug, Clone, Default)]
pub struct AuditLogFilter {
    pub actor_id: Option<Uuid>,
    pub action: Option<String>,
    pub target_type: Option<String>,
    pub target_id: Option<String>,
    pub since: Option<DateTime<Utc>>,
    pub until: Option<DateTime<Utc>>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

// ---------------------------------------------------------------------------
// Einladungen
// ---------------------------------------------------------------------------

/// Einladung-Datensatz
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EinladungRecord {
    pub id: Uuid,
    pub code: String,
    pub channel_id: Option<Uuid>,
    pub assigned_group_id: Option<Uuid>,
    pub max_uses: i64,
    pub used_count: i64,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
}

/// Daten zum Erstellen einer Einladung
#[derive(Debug, Clone)]
pub struct NeueEinladung<'a> {
    pub code: &'a str,
    pub channel_id: Option<Uuid>,
    pub assigned_group_id: Option<Uuid>,
    pub max_uses: i64,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_by: Uuid,
}
