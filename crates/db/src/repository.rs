//! Repository-Trait-Definitionen fuer Speakeasy
//!
//! Das Repository-Pattern entkoppelt die Geschaeftslogik von der konkreten
//! Datenbank-Implementierung. Alle Traits sind async und thread-safe.

use uuid::Uuid;

use crate::error::DbError;
use crate::models::{
    AuditLogFilter, AuditLogRecord, BanRecord, BenutzerRecord, BenutzerUpdate,
    BerechtigungsWert, BerechtigungsZiel, EffektiveBerechtigung, EinladungRecord,
    KanalGruppeRecord, KanalRecord, KanalUpdate, NeueEinladung, NeueKanalGruppe,
    NeuerKanal, NeuerBan, NeuerBenutzer, NeueServerGruppe, ServerGruppeRecord,
};

pub type DbResult<T> = Result<T, DbError>;

/// Unterstuetzte Datenbank-Backends
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DatabaseBackend {
    /// SQLite – Standard fuer Single-Instance-Betrieb
    Sqlite,
    /// PostgreSQL – fuer Multi-Instance-Betrieb
    Postgres,
}

impl std::fmt::Display for DatabaseBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sqlite => write!(f, "SQLite"),
            Self::Postgres => write!(f, "PostgreSQL"),
        }
    }
}

/// Konfiguration fuer die Datenbankverbindung
#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    /// Ausgewaehltes Backend
    pub backend: DatabaseBackend,
    /// Verbindungs-URL (z.B. "sqlite://speakeasy.db" oder "postgres://...")
    pub url: String,
    /// Maximale Anzahl gleichzeitiger Verbindungen im Pool
    pub max_verbindungen: u32,
    /// Ob WAL-Modus bei SQLite aktiviert werden soll
    pub sqlite_wal: bool,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            backend: DatabaseBackend::Sqlite,
            url: "sqlite://speakeasy.db".into(),
            max_verbindungen: 5,
            sqlite_wal: true,
        }
    }
}

// ---------------------------------------------------------------------------
// UserRepository
// ---------------------------------------------------------------------------

/// Repository fuer Benutzer-Datenzugriffe
#[allow(async_fn_in_trait)]
pub trait UserRepository: Send + Sync {
    /// Neuen Benutzer anlegen
    async fn create(&self, data: NeuerBenutzer<'_>) -> DbResult<BenutzerRecord>;

    /// Benutzer anhand seiner ID laden
    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<BenutzerRecord>>;

    /// Benutzer anhand seines Namens laden
    async fn get_by_name(&self, username: &str) -> DbResult<Option<BenutzerRecord>>;

    /// Benutzer aktualisieren
    async fn update(&self, id: Uuid, data: BenutzerUpdate) -> DbResult<BenutzerRecord>;

    /// Benutzer loeschen (weicher Loeschvorgang via is_active=false)
    async fn delete(&self, id: Uuid) -> DbResult<bool>;

    /// Alle Benutzer auflisten
    async fn list(&self, nur_aktive: bool) -> DbResult<Vec<BenutzerRecord>>;

    /// Benutzer authentifizieren: gibt None zurueck wenn Passwort falsch oder User nicht existiert
    async fn authenticate(
        &self,
        username: &str,
        password_hash: &str,
    ) -> DbResult<Option<BenutzerRecord>>;

    /// Letzten Login-Zeitstempel aktualisieren
    async fn update_last_login(&self, id: Uuid) -> DbResult<()>;
}

// ---------------------------------------------------------------------------
// ChannelRepository
// ---------------------------------------------------------------------------

/// Repository fuer Kanal-Datenzugriffe
#[allow(async_fn_in_trait)]
pub trait ChannelRepository: Send + Sync {
    /// Neuen Kanal anlegen
    async fn create(&self, data: NeuerKanal<'_>) -> DbResult<KanalRecord>;

    /// Kanal anhand seiner ID laden
    async fn get_by_id(&self, id: Uuid) -> DbResult<Option<KanalRecord>>;

    /// Alle Kanaele auflisten (flach, sortiert nach sort_order)
    async fn list(&self) -> DbResult<Vec<KanalRecord>>;

    /// Kanal aktualisieren
    async fn update(&self, id: Uuid, data: KanalUpdate) -> DbResult<KanalRecord>;

    /// Kanal loeschen (kaskadierend)
    async fn delete(&self, id: Uuid) -> DbResult<bool>;

    /// Unter-Kanaele eines Kanals laden
    async fn get_children(&self, parent_id: Uuid) -> DbResult<Vec<KanalRecord>>;

    /// Standard-Kanal ermitteln (is_default=true)
    async fn get_default(&self) -> DbResult<Option<KanalRecord>>;
}

// ---------------------------------------------------------------------------
// PermissionRepository
// ---------------------------------------------------------------------------

/// Repository fuer Berechtigungen
#[allow(async_fn_in_trait)]
pub trait PermissionRepository: Send + Sync {
    /// Alle Berechtigungen fuer ein Ziel laden
    async fn get_permissions(
        &self,
        ziel: &BerechtigungsZiel,
        channel_id: Option<Uuid>,
    ) -> DbResult<Vec<(String, BerechtigungsWert)>>;

    /// Eine Berechtigung setzen (upsert)
    async fn set_permission(
        &self,
        ziel: &BerechtigungsZiel,
        permission_key: &str,
        wert: BerechtigungsWert,
        channel_id: Option<Uuid>,
    ) -> DbResult<()>;

    /// Eine Berechtigung entfernen
    async fn remove_permission(
        &self,
        ziel: &BerechtigungsZiel,
        permission_key: &str,
        channel_id: Option<Uuid>,
    ) -> DbResult<bool>;

    /// Effektive Berechtigungen fuer einen User in einem Kanal aufloesen
    ///
    /// Aufloesung: Individual > Channel Group > Channel Default > Server Groups > Server Default
    /// Merge-Regel: Deny > Grant > Skip
    async fn resolve_effective_permissions(
        &self,
        user_id: Uuid,
        channel_id: Uuid,
    ) -> DbResult<Vec<EffektiveBerechtigung>>;
}

// ---------------------------------------------------------------------------
// ServerGroupRepository
// ---------------------------------------------------------------------------

/// Repository fuer Server-Gruppen
#[allow(async_fn_in_trait)]
pub trait ServerGroupRepository: Send + Sync {
    /// Neue Server-Gruppe anlegen
    async fn create(&self, data: NeueServerGruppe<'_>) -> DbResult<ServerGruppeRecord>;

    /// Server-Gruppe laden
    async fn get(&self, id: Uuid) -> DbResult<Option<ServerGruppeRecord>>;

    /// Alle Server-Gruppen auflisten
    async fn list(&self) -> DbResult<Vec<ServerGruppeRecord>>;

    /// Server-Gruppen eines Users laden
    async fn list_for_user(&self, user_id: Uuid) -> DbResult<Vec<ServerGruppeRecord>>;

    /// User zu Server-Gruppe hinzufuegen
    async fn add_member(&self, group_id: Uuid, user_id: Uuid) -> DbResult<()>;

    /// User aus Server-Gruppe entfernen
    async fn remove_member(&self, group_id: Uuid, user_id: Uuid) -> DbResult<bool>;

    /// Standard-Gruppe ermitteln
    async fn get_default(&self) -> DbResult<Option<ServerGruppeRecord>>;

    /// Server-Gruppe loeschen
    async fn delete(&self, id: Uuid) -> DbResult<bool>;
}

// ---------------------------------------------------------------------------
// ChannelGroupRepository
// ---------------------------------------------------------------------------

/// Repository fuer Kanal-Gruppen
#[allow(async_fn_in_trait)]
pub trait ChannelGroupRepository: Send + Sync {
    /// Neue Kanal-Gruppe anlegen
    async fn create(&self, data: NeueKanalGruppe<'_>) -> DbResult<KanalGruppeRecord>;

    /// Kanal-Gruppe laden
    async fn get(&self, id: Uuid) -> DbResult<Option<KanalGruppeRecord>>;

    /// Alle Kanal-Gruppen auflisten
    async fn list(&self) -> DbResult<Vec<KanalGruppeRecord>>;

    /// Kanal-Gruppe eines Users in einem Kanal ermitteln
    async fn get_for_user_in_channel(
        &self,
        user_id: Uuid,
        channel_id: Uuid,
    ) -> DbResult<Option<KanalGruppeRecord>>;

    /// Kanal-Gruppe eines Users in einem Kanal setzen (upsert)
    async fn set_member_group(
        &self,
        user_id: Uuid,
        channel_id: Uuid,
        group_id: Uuid,
    ) -> DbResult<()>;

    /// Kanal-Gruppen-Zuweisung aufheben
    async fn remove_member_group(
        &self,
        user_id: Uuid,
        channel_id: Uuid,
    ) -> DbResult<bool>;

    /// Kanal-Gruppe loeschen
    async fn delete(&self, id: Uuid) -> DbResult<bool>;
}

// ---------------------------------------------------------------------------
// BanRepository
// ---------------------------------------------------------------------------

/// Repository fuer Bans
#[allow(async_fn_in_trait)]
pub trait BanRepository: Send + Sync {
    /// Neuen Ban anlegen
    async fn create(&self, data: NeuerBan<'_>) -> DbResult<BanRecord>;

    /// Ban anhand seiner ID laden
    async fn get(&self, id: Uuid) -> DbResult<Option<BanRecord>>;

    /// Alle aktiven Bans auflisten
    async fn list(&self, nur_aktive: bool) -> DbResult<Vec<BanRecord>>;

    /// Ban entfernen (aufheben)
    async fn remove(&self, id: Uuid) -> DbResult<bool>;

    /// Pruefen ob ein User oder eine IP aktuell gebannt ist
    async fn is_banned(&self, user_id: Option<Uuid>, ip: Option<&str>) -> DbResult<Option<BanRecord>>;

    /// Abgelaufene Bans bereinigen
    async fn cleanup_expired(&self) -> DbResult<u64>;
}

// ---------------------------------------------------------------------------
// AuditLogRepository
// ---------------------------------------------------------------------------

/// Repository fuer das Audit-Log
#[allow(async_fn_in_trait)]
pub trait AuditLogRepository: Send + Sync {
    /// Ereignis protokollieren
    async fn log_event(
        &self,
        actor_id: Option<Uuid>,
        action: &str,
        target_type: Option<&str>,
        target_id: Option<&str>,
        details: serde_json::Value,
    ) -> DbResult<AuditLogRecord>;

    /// Ereignisse mit Filter auflisten
    async fn list_events(&self, filter: AuditLogFilter) -> DbResult<Vec<AuditLogRecord>>;

    /// Anzahl der Ereignisse zaehlen
    async fn count_events(&self, filter: AuditLogFilter) -> DbResult<i64>;
}

// ---------------------------------------------------------------------------
// InviteRepository
// ---------------------------------------------------------------------------

/// Repository fuer Einladungen
#[allow(async_fn_in_trait)]
pub trait InviteRepository: Send + Sync {
    /// Neue Einladung erstellen
    async fn create(&self, data: NeueEinladung<'_>) -> DbResult<EinladungRecord>;

    /// Einladung anhand ihrer ID laden
    async fn get(&self, id: Uuid) -> DbResult<Option<EinladungRecord>>;

    /// Einladung anhand ihres Codes laden
    async fn get_by_code(&self, code: &str) -> DbResult<Option<EinladungRecord>>;

    /// Alle Einladungen (optional gefiltert nach Ersteller)
    async fn list(&self, created_by: Option<Uuid>) -> DbResult<Vec<EinladungRecord>>;

    /// Einladung verwenden (used_count erhoehen, Gueltigkeit pruefen)
    ///
    /// Gibt None zurueck wenn die Einladung nicht existiert.
    /// Gibt Err(EinladungUngueltig) zurueck wenn abgelaufen.
    /// Gibt Err(EinladungErschoepft) zurueck wenn max_uses erreicht.
    async fn use_invite(&self, code: &str) -> DbResult<Option<EinladungRecord>>;

    /// Einladung widerrufen
    async fn revoke(&self, id: Uuid) -> DbResult<bool>;
}
