//! Event-Bus Trait-Definitionen
//!
//! Definiert die Schnittstelle fuer den internen Event-Bus.
//! Die Implementierung erfolgt im Server-Crate via tokio-Kanaelen.
//! Bei Multi-Instance-Betrieb kann dieser durch NATS oder PG NOTIFY ersetzt werden.

use crate::types::{ChannelId, ServerId, UserId};
use serde::{Deserialize, Serialize};

/// Alle systemweiten Ereignisse die ueber den Event-Bus fliessen
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SpeakeasyEvent {
    // --- Benutzer-Ereignisse ---
    /// Ein Benutzer hat sich verbunden
    BenutzerVerbunden {
        user_id: UserId,
        server_id: ServerId,
    },
    /// Ein Benutzer hat die Verbindung getrennt
    BenutzerGetrennt {
        user_id: UserId,
        server_id: ServerId,
        grund: String,
    },
    /// Ein Benutzer hat einen Kanal betreten
    KanalBetreten {
        user_id: UserId,
        kanal_id: ChannelId,
    },
    /// Ein Benutzer hat einen Kanal verlassen
    KanalVerlassen {
        user_id: UserId,
        kanal_id: ChannelId,
    },

    // --- Audio-Ereignisse ---
    /// Audio-Paket empfangen (wird nicht persistiert)
    AudioPaket {
        user_id: UserId,
        kanal_id: ChannelId,
        sequenz: u32,
    },

    // --- Server-Ereignisse ---
    /// Server-Konfiguration wurde geaendert
    KonfigurationGeaendert { server_id: ServerId },
}

/// Trait fuer den Event-Bus
///
/// Platzhalter-Trait – die konkrete Implementierung (tokio broadcast,
/// NATS, PG NOTIFY) wird spaeter im Server-Crate bereitgestellt.
pub trait EventBus: Send + Sync + 'static {
    /// Sendet ein Ereignis an alle Abonnenten
    fn senden(&self, event: SpeakeasyEvent) -> crate::Result<()>;

    /// Abonniert alle zukuenftigen Ereignisse
    ///
    /// Gibt einen Empfaenger zurueck. Die konkrete Implementierung
    /// bestimmt ob dies ein tokio::sync::broadcast::Receiver oder
    /// ein anderer Kanal-Typ ist.
    fn abonnieren(&self) -> Box<dyn EventEmpfaenger + Send>;
}

/// Empfaenger-Seite eines Event-Bus-Abonnements
pub trait EventEmpfaenger {
    /// Empfaengt das naechste Ereignis (blockierend in async-Kontext)
    fn empfangen(&mut self) -> Option<SpeakeasyEvent>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ServerId, UserId};

    #[test]
    fn event_ist_serde_kompatibel() {
        let uid = UserId::new();
        let sid = ServerId::new();
        let event = SpeakeasyEvent::BenutzerVerbunden {
            user_id: uid,
            server_id: sid,
        };
        let json = serde_json::to_string(&event).unwrap();
        let _: SpeakeasyEvent = serde_json::from_str(&json).unwrap();
    }
}
