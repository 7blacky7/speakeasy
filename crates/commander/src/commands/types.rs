//! Command- und Response-Typen fuer den einheitlichen Befehlsausführer

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Alle unterstuetzten Commander-Befehle
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    // --- Server ---
    /// Server-Informationen abrufen
    ServerInfo,
    /// Server-Konfiguration aendern
    ServerEdit {
        name: Option<String>,
        willkommensnachricht: Option<String>,
        max_clients: Option<u32>,
        host_nachricht: Option<String>,
    },
    /// Server stoppen
    ServerStop { grund: Option<String> },

    // --- Kanaele ---
    /// Kanalliste abrufen
    KanalListe,
    /// Kanal erstellen
    KanalErstellen {
        name: String,
        parent_id: Option<Uuid>,
        thema: Option<String>,
        passwort: Option<String>,
        max_clients: i64,
        sort_order: i64,
        permanent: bool,
    },
    /// Kanal bearbeiten
    KanalBearbeiten {
        id: Uuid,
        name: Option<String>,
        thema: Option<Option<String>>,
        max_clients: Option<i64>,
        sort_order: Option<i64>,
    },
    /// Kanal loeschen
    KanalLoeschen { id: Uuid },

    // --- Clients ---
    /// Liste verbundener Clients (nur ephemere Daten)
    ClientListe,
    /// Client kicken
    ClientKicken {
        client_id: Uuid,
        grund: Option<String>,
    },
    /// Client bannen
    ClientBannen {
        client_id: Uuid,
        dauer_secs: Option<u64>,
        grund: Option<String>,
        ip_bannen: bool,
    },
    /// Client in anderen Kanal verschieben
    ClientVerschieben { client_id: Uuid, kanal_id: Uuid },
    /// Client anpiken (Poke)
    ClientPoken { client_id: Uuid, nachricht: String },

    // --- Berechtigungen ---
    /// Berechtigungen fuer ein Ziel abfragen
    BerechtigungListe { ziel: String, scope: String },
    /// Berechtigung setzen
    BerechtigungSetzen {
        ziel: String,
        permission: String,
        wert: BerechtigungsWertInput,
        scope: String,
    },
    /// Berechtigung entfernen
    BerechtigungEntfernen {
        ziel: String,
        permission: String,
        scope: String,
    },

    // --- Dateien ---
    /// Dateien eines Kanals auflisten
    DateiListe { kanal_id: Uuid },
    /// Datei loeschen
    DateiLoeschen { datei_id: String },

    // --- Logs ---
    /// Audit-Log abfragen
    LogAbfragen {
        limit: u32,
        offset: u32,
        aktion_filter: Option<String>,
    },
}

impl Command {
    /// Gibt den erforderlichen Scope fuer API-Token-Authentifizierung zurueck.
    ///
    /// Session-Auth (nach Login) umgeht diese Pruefung.
    /// API-Tokens muessen den exakten Scope oder "cmd:*" besitzen.
    pub fn erforderlicher_scope(&self) -> &'static str {
        match self {
            // Lesende Server-Befehle
            Command::ServerInfo => "cmd:serverinfo",
            // Schreibende Server-Befehle
            Command::ServerEdit { .. } => "cmd:serveredit",
            Command::ServerStop { .. } => "cmd:serverstop",
            // Kanal-Lesebefehle
            Command::KanalListe => "cmd:channellist",
            // Kanal-Schreibbefehle
            Command::KanalErstellen { .. } => "cmd:channelcreate",
            Command::KanalBearbeiten { .. } => "cmd:channeledit",
            Command::KanalLoeschen { .. } => "cmd:channeldelete",
            // Client-Lesebefehle
            Command::ClientListe => "cmd:clientlist",
            // Client-Aktionsbefehle
            Command::ClientKicken { .. } => "cmd:clientkick",
            Command::ClientBannen { .. } => "cmd:clientban",
            Command::ClientVerschieben { .. } => "cmd:clientmove",
            Command::ClientPoken { .. } => "cmd:clientpoke",
            // Berechtigungs-Lesebefehle
            Command::BerechtigungListe { .. } => "cmd:permissionlist",
            // Berechtigungs-Schreibbefehle
            Command::BerechtigungSetzen { .. } => "cmd:permissionwrite",
            Command::BerechtigungEntfernen { .. } => "cmd:permissionwrite",
            // Datei-Befehle
            Command::DateiListe { .. } => "cmd:filelist",
            Command::DateiLoeschen { .. } => "cmd:filedelete",
            // Log-Befehle
            Command::LogAbfragen { .. } => "cmd:logview",
        }
    }

    /// Gibt true zurueck wenn dieser Befehl als "teuer" gilt
    /// (unterliegt strengerem Rate-Limiting).
    pub fn ist_teure_operation(&self) -> bool {
        matches!(
            self,
            Command::ClientBannen { .. }
                | Command::BerechtigungSetzen { .. }
                | Command::BerechtigungEntfernen { .. }
                | Command::ServerStop { .. }
        )
    }
}

/// Berechtigungswert-Eingabe (fuer REST/TCP-Deserialisierung)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum BerechtigungsWertInput {
    Grant,
    Deny,
    Skip,
    IntLimit(i64),
}

/// Antwort auf einen Commander-Befehl
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "typ")]
pub enum Response {
    /// Erfolg ohne Nutzlast
    Ok,
    /// Server-Informationen
    ServerInfo(ServerInfoResponse),
    /// Kanalliste
    KanalListe(Vec<KanalInfo>),
    /// Kanal-Detail
    Kanal(KanalInfo),
    /// Client-Liste
    ClientListe(Vec<ClientInfo>),
    /// Berechtigungsliste
    BerechtigungListe(Vec<BerechtigungsEintrag>),
    /// Dateiliste
    DateiListe(Vec<DateiEintrag>),
    /// Log-Eintraege
    LogEintraege(Vec<LogEintrag>),
}

/// Server-Informationen fuer Antworten
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfoResponse {
    pub name: String,
    pub willkommensnachricht: String,
    pub max_clients: u32,
    pub aktuelle_clients: u32,
    pub version: String,
    pub uptime_secs: u64,
}

/// Kanal-Informationen
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KanalInfo {
    pub id: Uuid,
    pub name: String,
    pub parent_id: Option<Uuid>,
    pub thema: Option<String>,
    pub max_clients: i64,
    pub aktuelle_clients: u32,
    pub sort_order: i64,
    pub passwort_geschuetzt: bool,
}

/// Client-Informationen (ephemer)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub user_id: Uuid,
    pub username: String,
    pub kanal_id: Option<Uuid>,
    pub verbunden_seit_ms: u64,
    pub ist_gemutet: bool,
    pub ist_gehoerlos: bool,
    pub ip_adresse: Option<String>,
}

/// Berechtigungs-Eintrag
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BerechtigungsEintrag {
    pub permission: String,
    pub wert: BerechtigungsWertInput,
}

/// Datei-Eintrag
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DateiEintrag {
    pub datei_id: String,
    pub name: String,
    pub groesse_bytes: u64,
    pub kanal_id: Uuid,
    pub hochgeladen_von: Uuid,
    pub hochgeladen_am_ms: u64,
    pub mime_typ: String,
}

/// Log-Eintrag
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEintrag {
    pub id: Uuid,
    pub aktor_id: Option<Uuid>,
    pub aktion: String,
    pub ziel_typ: Option<String>,
    pub ziel_id: Option<String>,
    pub zeitstempel: chrono::DateTime<chrono::Utc>,
    pub details: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_server_info_debug() {
        let cmd = Command::ServerInfo;
        let dbg = format!("{:?}", cmd);
        assert!(dbg.contains("ServerInfo"));
    }

    #[test]
    fn command_server_edit_felder() {
        let cmd = Command::ServerEdit {
            name: Some("Mein Server".into()),
            willkommensnachricht: None,
            max_clients: Some(50),
            host_nachricht: None,
        };
        assert!(matches!(cmd, Command::ServerEdit { .. }));
    }

    #[test]
    fn command_kanal_erstellen() {
        let cmd = Command::KanalErstellen {
            name: "General".into(),
            parent_id: None,
            thema: Some("Allgemeiner Kanal".into()),
            passwort: None,
            max_clients: 0,
            sort_order: 0,
            permanent: true,
        };
        if let Command::KanalErstellen { name, .. } = cmd {
            assert_eq!(name, "General");
        }
    }

    #[test]
    fn response_serialisierung() {
        let resp = Response::ServerInfo(ServerInfoResponse {
            name: "Test".into(),
            willkommensnachricht: "Willkommen".into(),
            max_clients: 32,
            aktuelle_clients: 5,
            version: "0.1.0".into(),
            uptime_secs: 3600,
        });
        let json = serde_json::to_string(&resp).expect("Serialisierung fehlgeschlagen");
        assert!(json.contains("Test"));
        assert!(json.contains("3600"));
    }

    #[test]
    fn berechtigung_wert_serialisierung() {
        let wert = BerechtigungsWertInput::Grant;
        let json = serde_json::to_string(&wert).unwrap();
        assert!(json.contains("Grant"));
    }

    #[test]
    fn log_eintrag_felder() {
        let eintrag = LogEintrag {
            id: Uuid::new_v4(),
            aktor_id: None,
            aktion: "kanal.erstellt".into(),
            ziel_typ: Some("channel".into()),
            ziel_id: Some(Uuid::new_v4().to_string()),
            zeitstempel: chrono::Utc::now(),
            details: serde_json::json!({"name": "General"}),
        };
        assert_eq!(eintrag.aktion, "kanal.erstellt");
    }
}
