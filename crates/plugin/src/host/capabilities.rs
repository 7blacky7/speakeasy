//! Capability Model – was ein Plugin darf

use crate::manifest::Capabilities;

/// Prueft ob eine bestimmte Faehigkeit in den Capabilities aktiviert ist
pub fn hat_faehigkeit(caps: &Capabilities, faehigkeit: &str) -> bool {
    match faehigkeit {
        "filesystem" => caps.filesystem,
        "network" => caps.network,
        "audio_read" => caps.audio_read,
        "audio_write" => caps.audio_write,
        "chat_read" => caps.chat_read,
        "chat_write" => caps.chat_write,
        "user_management" => caps.user_management,
        "server_config" => caps.server_config,
        _ => false,
    }
}

/// Gibt alle aktivierten Capabilities als String-Liste zurueck
pub fn aktivierte_faehigkeiten(caps: &Capabilities) -> Vec<&'static str> {
    let mut liste = Vec::new();
    if caps.filesystem {
        liste.push("filesystem");
    }
    if caps.network {
        liste.push("network");
    }
    if caps.audio_read {
        liste.push("audio_read");
    }
    if caps.audio_write {
        liste.push("audio_write");
    }
    if caps.chat_read {
        liste.push("chat_read");
    }
    if caps.chat_write {
        liste.push("chat_write");
    }
    if caps.user_management {
        liste.push("user_management");
    }
    if caps.server_config {
        liste.push("server_config");
    }
    liste
}

/// Prueft ob eine Capability-Anfrage mit den deklarierten Capabilities uebereinstimmt
pub fn capabilities_pruefen(deklariert: &Capabilities, benoetigt: &[&str]) -> Result<(), String> {
    for cap in benoetigt {
        if !hat_faehigkeit(deklariert, cap) {
            return Err(format!("Capability '{}' nicht deklariert", cap));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::Capabilities;

    fn test_caps() -> Capabilities {
        Capabilities {
            chat_read: true,
            chat_write: true,
            ..Default::default()
        }
    }

    #[test]
    fn hat_faehigkeit_chat_read() {
        let caps = test_caps();
        assert!(hat_faehigkeit(&caps, "chat_read"));
        assert!(hat_faehigkeit(&caps, "chat_write"));
        assert!(!hat_faehigkeit(&caps, "network"));
        assert!(!hat_faehigkeit(&caps, "filesystem"));
    }

    #[test]
    fn hat_faehigkeit_unbekannte_cap() {
        let caps = test_caps();
        assert!(!hat_faehigkeit(&caps, "unbekannt"));
    }

    #[test]
    fn aktivierte_faehigkeiten_liste() {
        let caps = test_caps();
        let liste = aktivierte_faehigkeiten(&caps);
        assert!(liste.contains(&"chat_read"));
        assert!(liste.contains(&"chat_write"));
        assert!(!liste.contains(&"network"));
    }

    #[test]
    fn capabilities_pruefen_ok() {
        let caps = test_caps();
        assert!(capabilities_pruefen(&caps, &["chat_read", "chat_write"]).is_ok());
    }

    #[test]
    fn capabilities_pruefen_fehlend() {
        let caps = test_caps();
        let err = capabilities_pruefen(&caps, &["chat_read", "network"]).unwrap_err();
        assert!(err.contains("network"));
    }

    #[test]
    fn alle_caps_deaktiviert() {
        let caps = Capabilities::default();
        let liste = aktivierte_faehigkeiten(&caps);
        assert!(liste.is_empty());
    }

    #[test]
    fn alle_caps_aktiviert() {
        let caps = Capabilities {
            filesystem: true,
            network: true,
            audio_read: true,
            audio_write: true,
            chat_read: true,
            chat_write: true,
            user_management: true,
            server_config: true,
        };
        let liste = aktivierte_faehigkeiten(&caps);
        assert_eq!(liste.len(), 8);
    }
}
