//! Event-System fuer Plugins
//!
//! Definiert alle Events die ein Plugin abonnieren kann
//! sowie Hook-Ergebnisse die den Ablauf steuern koennen.

use serde::{Deserialize, Serialize};

/// Alle Events die das Plugin-System ausstrahlt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginEvent {
    /// Benutzer tritt einem Kanal bei
    UserJoin { user_id: String, channel_id: String },
    /// Benutzer verlaesst einen Kanal
    UserLeave {
        user_id: String,
        channel_id: String,
        reason: String,
    },
    /// Chat-Nachricht wird gesendet
    ChatMessage {
        channel_id: String,
        sender_id: String,
        content: String,
    },
    /// Sprach-Uebertragung beginnt
    VoiceStart { user_id: String, channel_id: String },
    /// Sprach-Uebertragung endet
    VoiceStop { user_id: String, channel_id: String },
    /// Server wird gestartet
    ServerStart,
    /// Server wird gestoppt
    ServerStop,
    /// Kanal wird erstellt
    ChannelCreate { channel_id: String, name: String },
    /// Kanal wird geloescht
    ChannelDelete { channel_id: String },
}

impl PluginEvent {
    /// Gibt den Event-Namen als String zurueck (fuer Abonnement-Vergleich)
    pub fn name(&self) -> &'static str {
        match self {
            Self::UserJoin { .. } => "user_join",
            Self::UserLeave { .. } => "user_leave",
            Self::ChatMessage { .. } => "chat_message",
            Self::VoiceStart { .. } => "voice_start",
            Self::VoiceStop { .. } => "voice_stop",
            Self::ServerStart => "server_start",
            Self::ServerStop => "server_stop",
            Self::ChannelCreate { .. } => "channel_create",
            Self::ChannelDelete { .. } => "channel_delete",
        }
    }
}

/// Ergebnis eines Hook-Aufrufs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HookResult {
    /// Aktion erlauben – normaler Ablauf
    Allow,
    /// Aktion verhindern mit Begruendung
    Deny { reason: String },
    /// Daten modifizieren (z.B. Chat-Nachricht veraendern)
    Modify { data: Vec<u8> },
}

impl HookResult {
    /// Gibt true zurueck wenn die Aktion erlaubt ist
    pub fn ist_erlaubt(&self) -> bool {
        matches!(self, Self::Allow | Self::Modify { .. })
    }

    /// Gibt den Ablehnungsgrund zurueck falls vorhanden
    pub fn ablehnungsgrund(&self) -> Option<&str> {
        match self {
            Self::Deny { reason } => Some(reason.as_str()),
            _ => None,
        }
    }
}

/// Kombiniert mehrere Hook-Ergebnisse nach der Regel: erstes Deny gewinnt
pub fn hook_ergebnisse_kombinieren(ergebnisse: Vec<HookResult>) -> HookResult {
    for ergebnis in ergebnisse {
        if let HookResult::Deny { .. } = &ergebnis {
            return ergebnis;
        }
    }
    // Letztes Modify gewinnt, sonst Allow
    HookResult::Allow
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_name_user_join() {
        let e = PluginEvent::UserJoin {
            user_id: "u1".into(),
            channel_id: "c1".into(),
        };
        assert_eq!(e.name(), "user_join");
    }

    #[test]
    fn event_name_chat_message() {
        let e = PluginEvent::ChatMessage {
            channel_id: "c1".into(),
            sender_id: "u1".into(),
            content: "Hallo".into(),
        };
        assert_eq!(e.name(), "chat_message");
    }

    #[test]
    fn event_name_server_start() {
        assert_eq!(PluginEvent::ServerStart.name(), "server_start");
    }

    #[test]
    fn hook_result_allow_ist_erlaubt() {
        assert!(HookResult::Allow.ist_erlaubt());
    }

    #[test]
    fn hook_result_deny_nicht_erlaubt() {
        let r = HookResult::Deny {
            reason: "Spam".into(),
        };
        assert!(!r.ist_erlaubt());
        assert_eq!(r.ablehnungsgrund(), Some("Spam"));
    }

    #[test]
    fn hook_result_modify_ist_erlaubt() {
        let r = HookResult::Modify {
            data: vec![1, 2, 3],
        };
        assert!(r.ist_erlaubt());
        assert!(r.ablehnungsgrund().is_none());
    }

    #[test]
    fn kombinieren_erstes_deny_gewinnt() {
        let ergebnisse = vec![
            HookResult::Allow,
            HookResult::Deny {
                reason: "Verboten".into(),
            },
            HookResult::Allow,
        ];
        let r = hook_ergebnisse_kombinieren(ergebnisse);
        assert!(matches!(r, HookResult::Deny { .. }));
    }

    #[test]
    fn kombinieren_alle_allow() {
        let ergebnisse = vec![HookResult::Allow, HookResult::Allow];
        let r = hook_ergebnisse_kombinieren(ergebnisse);
        assert!(matches!(r, HookResult::Allow));
    }

    #[test]
    fn kombinieren_leer_ergibt_allow() {
        let r = hook_ergebnisse_kombinieren(vec![]);
        assert!(matches!(r, HookResult::Allow));
    }

    #[test]
    fn event_serde() {
        let e = PluginEvent::UserLeave {
            user_id: "u1".into(),
            channel_id: "c1".into(),
            reason: "Disconnect".into(),
        };
        let json = serde_json::to_string(&e).unwrap();
        let e2: PluginEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(e2.name(), "user_leave");
    }
}
