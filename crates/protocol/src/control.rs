//! Control-Protokoll-Nachrichten
//!
//! Definiert alle Steuerungsnachrichten die ueber die TCP/TLS-Verbindung
//! zwischen Client und Server ausgetauscht werden (Signaling, Auth, etc.).

use serde::{Deserialize, Serialize};
use speakeasy_core::types::{ChannelId, ServerId, UserId};

/// Typ einer Protokoll-Nachricht (Richtung und Kategorie)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageType {
    /// Anfrage vom Client an den Server
    Anfrage,
    /// Antwort vom Server an den Client
    Antwort,
    /// Einseitige Benachrichtigung (Client oder Server)
    Benachrichtigung,
    /// Fehler-Antwort
    Fehler,
}

/// Alle unterstuetzten Befehle im Control-Protokoll
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommandType {
    // --- Verbindung ---
    /// Initiales Handshake inkl. Protokollversion
    Handshake,
    /// Heartbeat / Keep-Alive
    Ping,
    /// Antwort auf Ping
    Pong,
    /// Verbindung sauber trennen
    Trennen,

    // --- Authentifizierung ---
    /// Login mit Passwort oder Token
    Login,
    /// Logout und Session-Invalidierung
    Logout,
    /// Token erneuern
    TokenErneuern,

    // --- Kanaele ---
    /// Liste aller Kanaele abrufen
    KanalListe,
    /// Einen Kanal betreten
    KanalBetreten,
    /// Einen Kanal verlassen
    KanalVerlassen,
    /// Kanal erstellen (Moderator+)
    KanalErstellen,
    /// Kanal loeschen (Moderator+)
    KanalLoeschen,

    // --- Benutzer ---
    /// Informationen ueber einen Benutzer abrufen
    BenutzerInfo,
    /// Eigene Informationen aktualisieren
    BenutzerAktualisieren,

    // --- Server ---
    /// Server-Informationen abrufen
    ServerInfo,

    // --- Admin ---
    /// Benutzer aus dem Server entfernen (Admin)
    Kicken,
    /// Benutzer dauerhaft sperren (Admin)
    Bannen,
}

/// Versionsinformationen fuer den Handshake
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtokollVersion {
    /// Haupt-Versionsnummer (inkompatible Aenderungen)
    pub major: u16,
    /// Neben-Versionsnummer (abwaertskompatible Aenderungen)
    pub minor: u16,
}

impl ProtokollVersion {
    pub const AKTUELL: Self = Self { major: 1, minor: 0 };
}

/// Eine Control-Nachricht im Speakeasy-Protokoll
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlMessage {
    /// Eindeutige Nachrichten-ID fuer Request/Response-Zuordnung
    pub nachricht_id: u64,
    /// Typ der Nachricht
    pub nachricht_typ: MessageType,
    /// Befehl
    pub befehl: CommandType,
    /// Optionaler Absender (wird vom Server gesetzt)
    pub absender: Option<UserId>,
    /// Optionaler Zielserver
    pub server_id: Option<ServerId>,
    /// Optionaler Zielkanal
    pub kanal_id: Option<ChannelId>,
    /// Nutzlast als JSON-Wert (befehlsspezifisch)
    pub nutzlast: serde_json::Value,
}

impl ControlMessage {
    /// Erstellt eine neue Anfrage-Nachricht
    pub fn anfrage(id: u64, befehl: CommandType, nutzlast: serde_json::Value) -> Self {
        Self {
            nachricht_id: id,
            nachricht_typ: MessageType::Anfrage,
            befehl,
            absender: None,
            server_id: None,
            kanal_id: None,
            nutzlast,
        }
    }

    /// Erstellt eine Antwort auf eine bestehende Nachricht
    pub fn antwort(anfrage_id: u64, nutzlast: serde_json::Value) -> Self {
        Self {
            nachricht_id: anfrage_id,
            nachricht_typ: MessageType::Antwort,
            befehl: CommandType::Ping, // Platzhalter – wird durch konkreten Befehl ersetzt
            absender: None,
            server_id: None,
            kanal_id: None,
            nutzlast,
        }
    }

    /// Erstellt eine Fehler-Antwort
    pub fn fehler(anfrage_id: u64, meldung: impl Into<String>) -> Self {
        Self {
            nachricht_id: anfrage_id,
            nachricht_typ: MessageType::Fehler,
            befehl: CommandType::Ping,
            absender: None,
            server_id: None,
            kanal_id: None,
            nutzlast: serde_json::json!({ "fehler": meldung.into() }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_message_serde() {
        let msg = ControlMessage::anfrage(
            1,
            CommandType::Ping,
            serde_json::Value::Null,
        );
        let json = serde_json::to_string(&msg).unwrap();
        let msg2: ControlMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg.nachricht_id, msg2.nachricht_id);
        assert_eq!(msg.befehl, msg2.befehl);
    }

    #[test]
    fn fehler_nachricht_enthaelt_meldung() {
        let msg = ControlMessage::fehler(42, "Verbindung abgelehnt");
        assert_eq!(msg.nachricht_typ, MessageType::Fehler);
        assert_eq!(msg.nutzlast["fehler"], "Verbindung abgelehnt");
    }

    #[test]
    fn protokoll_version_aktuell() {
        assert_eq!(ProtokollVersion::AKTUELL.major, 1);
        assert_eq!(ProtokollVersion::AKTUELL.minor, 0);
    }
}
