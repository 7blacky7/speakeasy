//! DTLS/E2E Kryptografie-Typen
//!
//! Definiert die Typen fuer den Krypto-Handshake und das Key-Management.
//! Die eigentliche Implementierung der Kryptografie erfolgt in Phase 5.
//!
//! ## Design
//! - `CryptoMode`: Legt den Verschluesselungsmodus pro Kanal fest
//! - `KeyExchangeMessage`: DTLS-Handshake zwischen Client und Server
//! - `E2EKeyMessage`: End-to-End Gruppenschluessel-Verwaltung

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// CryptoMode
// ---------------------------------------------------------------------------

/// Verschluesselungsmodus fuer einen Voice-Kanal
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CryptoMode {
    /// Keine Verschluesselung (nur fuer Tests/interne Netzwerke)
    #[default]
    None,
    /// DTLS – Transport-Verschluesselung (Server sieht Audio)
    Dtls,
    /// End-to-End Verschluesselung (Server forwardet blind, SFU-Stil)
    E2E,
}

impl std::fmt::Display for CryptoMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CryptoMode::None => write!(f, "none"),
            CryptoMode::Dtls => write!(f, "dtls"),
            CryptoMode::E2E => write!(f, "e2e"),
        }
    }
}

impl std::str::FromStr for CryptoMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "none" => Ok(Self::None),
            "dtls" => Ok(Self::Dtls),
            "e2e" => Ok(Self::E2E),
            other => Err(format!("Unbekannter CryptoMode: '{}'", other)),
        }
    }
}

// ---------------------------------------------------------------------------
// DTLS-Handshake Nachrichtentypen
// ---------------------------------------------------------------------------

/// Algorithmus fuer den Schluessel-Austausch
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum KeyExchangeAlgorithm {
    /// X25519 Diffie-Hellman
    #[default]
    X25519,
    /// P-256 Elliptic Curve Diffie-Hellman
    P256,
}

/// AEAD-Verschluesselungsalgorithmus nach dem Key-Exchange
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AeadAlgorithm {
    /// AES-128-GCM
    Aes128Gcm,
    /// AES-256-GCM
    #[default]
    Aes256Gcm,
    /// ChaCha20-Poly1305
    ChaCha20Poly1305,
}

/// DTLS-Handshake Nachricht (Client <-> Server)
///
/// Modelliert die Phasen des DTLS-Handshakes als typsicheres Enum.
/// Die eigentliche kryptografische Verarbeitung erfolgt in Phase 5.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "step", rename_all = "snake_case")]
pub enum KeyExchangeMessage {
    /// Schritt 1: Client initiiert den Handshake
    ClientHello {
        /// Vom Client unterstuetzte Algorithmen (bevorzugte zuerst)
        supported_key_exchange: Vec<KeyExchangeAlgorithm>,
        /// Vom Client unterstuetzte AEAD-Algorithmen
        supported_aead: Vec<AeadAlgorithm>,
        /// Zufaelliger Nonce des Clients (32 Bytes, Base64)
        client_random: String,
        /// DTLS-Protokollversion
        protocol_version: String,
    },

    /// Schritt 2: Server antwortet mit gewaehlten Algorithmen
    ServerHello {
        /// Gewaehlter Key-Exchange-Algorithmus
        key_exchange: KeyExchangeAlgorithm,
        /// Gewaehlter AEAD-Algorithmus
        aead: AeadAlgorithm,
        /// Zufaelliger Nonce des Servers (32 Bytes, Base64)
        server_random: String,
        /// Oeffentlicher Schluessel des Servers (Base64)
        server_public_key: String,
        /// Zertifikat-Fingerprint (SHA-256 hex, fuer Verifikation)
        certificate_fingerprint: String,
    },

    /// Schritt 3: Client sendet seinen oeffentlichen Schluessel
    KeyExchange {
        /// Oeffentlicher Schluessel des Clients (Base64)
        client_public_key: String,
        /// Verschluesselter Verify-Wert (beweist Besitz des privaten Schluessels)
        encrypted_verify: String,
    },

    /// Schritt 4: Server bestaetigt den Handshake
    KeyConfirm {
        /// Bestaetigung (HMAC ueber beide Randoms + abgeleiteter Sitzungsschluessel)
        server_finished: String,
        /// Zugewiesene SSRC fuer verschluesselte Pakete
        ssrc: u32,
    },
}

// ---------------------------------------------------------------------------
// E2E Gruppenschluessel-Nachrichten
// ---------------------------------------------------------------------------

/// Zweck eines Gruppenschluessels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyPurpose {
    /// Audio-Verschluesselung
    Audio,
    /// Text/Datei-Verschluesselung
    Text,
}

/// E2E Gruppenschluessel-Nachricht
///
/// Wird verwendet um Gruppenschluessel sicher an alle Kanal-Mitglieder
/// zu verteilen. Der Server sieht nur verschluesselte Schluessel-Blobs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum E2EKeyMessage {
    /// Gruppenschluessel an alle Mitglieder verteilen
    ///
    /// Der Sender verschluesselt den Gruppenschluessel fuer jeden
    /// Empfaenger einzeln mit dessen oeffentlichem Schluessel.
    GroupKeyDistribute {
        /// Eindeutige Schluessel-ID (monoton steigend)
        key_id: u64,
        /// Zweck des Schluessels
        purpose: KeyPurpose,
        /// Map: user_id (String) -> verschluesselter Schluessel (Base64)
        encrypted_keys: std::collections::HashMap<String, String>,
        /// Algorithmus mit dem der Gruppenschluessel geschuetzt ist
        wrapping_algorithm: AeadAlgorithm,
        /// Zeitpunkt ab dem der Schluessel gueltig ist (Unix-ms)
        valid_from_ms: u64,
        /// Zeitpunkt bis zu dem der Schluessel gueltig ist (Unix-ms, 0 = kein Ablauf)
        expires_at_ms: u64,
    },

    /// Schluessel-Rotation anfordern
    ///
    /// Wird gesendet wenn ein Mitglied den Kanal verlaesst oder
    /// wenn der aktuelle Schluessel abgelaufen ist.
    KeyRotationRequest {
        /// Aktuell verwendete Schluessel-ID
        current_key_id: u64,
        /// Grund fuer die Rotation
        reason: KeyRotationReason,
    },

    /// Schluessel zurueckziehen (fuer ausgetretene Mitglieder)
    ///
    /// Nach Erhalt dieser Nachricht darf kein neues Material mit
    /// dem angegebenen Schluessel verschluesselt werden.
    KeyRevoke {
        /// Zurueckzuziehende Schluessel-ID
        key_id: u64,
        /// Unix-Timestamp ab dem der Schluessel ungueltig ist
        revoked_at_ms: u64,
        /// Grund fuer den Widerruf
        reason: String,
    },
}

/// Grund fuer eine Schluessel-Rotation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyRotationReason {
    /// Planmaessige Rotation (timer-basiert)
    Scheduled,
    /// Mitglied hat Kanal verlassen
    MemberLeft,
    /// Mitglied wurde gekickt/gebannt
    MemberRemoved,
    /// Manuell durch Admin angefordert
    ManualRequest,
    /// Schluessel ist abgelaufen
    KeyExpired,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crypto_mode_display_und_parse() {
        assert_eq!(CryptoMode::None.to_string(), "none");
        assert_eq!(CryptoMode::Dtls.to_string(), "dtls");
        assert_eq!(CryptoMode::E2E.to_string(), "e2e");

        let parsed: CryptoMode = "dtls".parse().unwrap();
        assert_eq!(parsed, CryptoMode::Dtls);

        let err = "invalid".parse::<CryptoMode>();
        assert!(err.is_err());
    }

    #[test]
    fn crypto_mode_default_ist_none() {
        assert_eq!(CryptoMode::default(), CryptoMode::None);
    }

    #[test]
    fn key_exchange_client_hello_serialisierung() {
        let msg = KeyExchangeMessage::ClientHello {
            supported_key_exchange: vec![KeyExchangeAlgorithm::X25519, KeyExchangeAlgorithm::P256],
            supported_aead: vec![AeadAlgorithm::Aes256Gcm],
            client_random: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".to_string(),
            protocol_version: "DTLS1.3".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let decoded: KeyExchangeMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(decoded, KeyExchangeMessage::ClientHello { .. }));
    }

    #[test]
    fn key_exchange_alle_schritte_serialisierbar() {
        let msgs = vec![
            KeyExchangeMessage::ClientHello {
                supported_key_exchange: vec![KeyExchangeAlgorithm::X25519],
                supported_aead: vec![AeadAlgorithm::ChaCha20Poly1305],
                client_random: "rand1".to_string(),
                protocol_version: "DTLS1.3".to_string(),
            },
            KeyExchangeMessage::ServerHello {
                key_exchange: KeyExchangeAlgorithm::X25519,
                aead: AeadAlgorithm::ChaCha20Poly1305,
                server_random: "rand2".to_string(),
                server_public_key: "pubkey".to_string(),
                certificate_fingerprint: "AA:BB:CC".to_string(),
            },
            KeyExchangeMessage::KeyExchange {
                client_public_key: "clientpub".to_string(),
                encrypted_verify: "verify".to_string(),
            },
            KeyExchangeMessage::KeyConfirm {
                server_finished: "finished".to_string(),
                ssrc: 0xDEADBEEF,
            },
        ];

        for msg in &msgs {
            let json = serde_json::to_string(msg).unwrap();
            let _: KeyExchangeMessage = serde_json::from_str(&json).unwrap();
        }
    }

    #[test]
    fn e2e_group_key_distribute_serialisierung() {
        let mut keys = std::collections::HashMap::new();
        keys.insert(
            "user-123".to_string(),
            "encrypted_key_blob_base64".to_string(),
        );
        keys.insert("user-456".to_string(), "another_encrypted_key".to_string());

        let msg = E2EKeyMessage::GroupKeyDistribute {
            key_id: 1,
            purpose: KeyPurpose::Audio,
            encrypted_keys: keys,
            wrapping_algorithm: AeadAlgorithm::Aes256Gcm,
            valid_from_ms: 1000000,
            expires_at_ms: 2000000,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let decoded: E2EKeyMessage = serde_json::from_str(&json).unwrap();
        if let E2EKeyMessage::GroupKeyDistribute {
            key_id, purpose, ..
        } = decoded
        {
            assert_eq!(key_id, 1);
            assert_eq!(purpose, KeyPurpose::Audio);
        } else {
            panic!("Falscher Typ");
        }
    }

    #[test]
    fn e2e_key_rotation_request_serialisierung() {
        let msg = E2EKeyMessage::KeyRotationRequest {
            current_key_id: 42,
            reason: KeyRotationReason::MemberLeft,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let decoded: E2EKeyMessage = serde_json::from_str(&json).unwrap();
        if let E2EKeyMessage::KeyRotationRequest {
            current_key_id,
            reason,
        } = decoded
        {
            assert_eq!(current_key_id, 42);
            assert_eq!(reason, KeyRotationReason::MemberLeft);
        } else {
            panic!("Falscher Typ");
        }
    }

    #[test]
    fn e2e_key_revoke_serialisierung() {
        let msg = E2EKeyMessage::KeyRevoke {
            key_id: 7,
            revoked_at_ms: 9999999,
            reason: "Mitglied hat Kanal verlassen".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let decoded: E2EKeyMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(
            decoded,
            E2EKeyMessage::KeyRevoke { key_id: 7, .. }
        ));
    }
}
