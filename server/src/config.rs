//! Server-Konfiguration
//!
//! Wird beim Start aus einer TOML-Datei geladen. Alle Felder haben
//! sinnvolle Standardwerte, sodass der Server ohne Konfigurationsdatei
//! lauffaehig ist.

use serde::{Deserialize, Serialize};

/// Vollstaendige Server-Konfiguration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct ServerConfig {
    /// Allgemeine Server-Einstellungen
    pub server: ServerEinstellungen,
    /// Netzwerk-Einstellungen
    pub netzwerk: NetzwerkEinstellungen,
    /// Datenbank-Einstellungen
    pub datenbank: DatenbankEinstellungen,
    /// Audio/Voice-Einstellungen
    pub audio: AudioEinstellungen,
    /// Logging-Einstellungen
    pub logging: LoggingEinstellungen,
    /// Commander-Einstellungen (REST, TCP/TLS, gRPC)
    pub commander: CommanderEinstellungen,
    /// Observability-Einstellungen (Metriken, Health)
    pub observability: ObservabilityEinstellungen,
    /// Plugin-Einstellungen
    pub plugins: PluginEinstellungen,
}

/// Allgemeine Server-Einstellungen
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerEinstellungen {
    /// Anzeigename des Servers
    pub name: String,
    /// Maximale Anzahl gleichzeitiger Clients
    pub max_clients: u32,
    /// Willkommensnachricht (optional)
    pub willkommen: Option<String>,
    /// Server-Passwort (leer = kein Passwort)
    pub passwort: Option<String>,
}

impl Default for ServerEinstellungen {
    fn default() -> Self {
        Self {
            name: "Speakeasy Server".into(),
            max_clients: 512,
            willkommen: None,
            passwort: None,
        }
    }
}

/// Netzwerk-Einstellungen
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct NetzwerkEinstellungen {
    /// Bind-Adresse fuer die TCP/TLS-Verbindung (Control-Protokoll)
    pub bind_adresse: String,
    /// Port fuer die TCP/TLS-Verbindung
    pub tcp_port: u16,
    /// Port fuer UDP (Voice-Daten)
    pub udp_port: u16,
    /// Port fuer die REST-API
    pub api_port: u16,
    /// Port fuer gRPC
    pub grpc_port: u16,
    /// TLS-Zertifikat-Pfad (leer = kein TLS im Entwicklungsmodus)
    pub tls_zertifikat: Option<String>,
    /// TLS-Schluessel-Pfad
    pub tls_schluessel: Option<String>,
}

impl Default for NetzwerkEinstellungen {
    fn default() -> Self {
        Self {
            bind_adresse: "0.0.0.0".into(),
            tcp_port: 9987,
            udp_port: 9987,
            api_port: 10080,
            grpc_port: 10443,
            tls_zertifikat: None,
            tls_schluessel: None,
        }
    }
}

/// Datenbank-Einstellungen
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DatenbankEinstellungen {
    /// Datenbank-Typ: "sqlite" oder "postgres"
    pub typ: String,
    /// Verbindungs-URL
    pub url: String,
    /// Maximale Verbindungspool-Groesse
    pub max_verbindungen: u32,
}

impl Default for DatenbankEinstellungen {
    fn default() -> Self {
        Self {
            typ: "sqlite".into(),
            url: "sqlite://speakeasy.db".into(),
            max_verbindungen: 5,
        }
    }
}

/// Audio/Voice-Einstellungen
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioEinstellungen {
    /// Maximale Bitrate pro Client in kbit/s
    pub max_bitrate_kbps: u32,
    /// Jitter-Buffer-Groesse in Millisekunden
    pub jitter_buffer_ms: u32,
    /// Maximale Stille-Erkennungszeit in ms bevor Client gemuted wird
    pub stille_timeout_ms: u32,
}

impl Default for AudioEinstellungen {
    fn default() -> Self {
        Self {
            max_bitrate_kbps: 128,
            jitter_buffer_ms: 60,
            stille_timeout_ms: 300,
        }
    }
}

/// Logging-Einstellungen
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LoggingEinstellungen {
    /// Log-Level: "trace", "debug", "info", "warn", "error"
    pub level: String,
    /// Format: "json" oder "text"
    pub format: String,
    /// Log-Datei-Pfad (leer = nur stdout)
    pub datei: Option<String>,
}

impl Default for LoggingEinstellungen {
    fn default() -> Self {
        Self {
            level: "info".into(),
            format: "text".into(),
            datei: None,
        }
    }
}

/// Commander-Einstellungen (REST, TCP/TLS, gRPC Verwaltungsschnittstellen)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CommanderEinstellungen {
    /// Port fuer die REST-API (Standard: 8080)
    pub rest_port: u16,
    /// Port fuer TCP/TLS ServerQuery (Standard: 10011)
    pub tcp_port: u16,
    /// Maximale TCP-Verbindungen
    pub tcp_max_verbindungen: usize,
    /// CORS-Origins fuer REST (leer = alle erlaubt)
    pub cors_origins: Vec<String>,
}

impl Default for CommanderEinstellungen {
    fn default() -> Self {
        Self {
            rest_port: 8080,
            tcp_port: 10011,
            tcp_max_verbindungen: 100,
            cors_origins: vec![],
        }
    }
}

/// Observability-Einstellungen (Metriken + Health-Check)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ObservabilityEinstellungen {
    /// Aktiviert den Observability-Server
    pub aktiviert: bool,
    /// Port fuer Metriken und Health (Standard: 9300)
    pub port: u16,
}

impl Default for ObservabilityEinstellungen {
    fn default() -> Self {
        Self {
            aktiviert: true,
            port: 9300,
        }
    }
}

/// Plugin-Einstellungen
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PluginEinstellungen {
    /// Aktiviert das Plugin-System
    pub aktiviert: bool,
    /// Verzeichnis fuer Plugin-Dateien (optional)
    pub verzeichnis: Option<String>,
}

impl ServerConfig {
    /// Laedt die Konfiguration aus einer TOML-Datei.
    /// Gibt die Standardkonfiguration zurueck wenn die Datei nicht existiert.
    pub fn laden(pfad: &str) -> anyhow::Result<Self> {
        match std::fs::read_to_string(pfad) {
            Ok(inhalt) => {
                let config: Self = toml::from_str(&inhalt)
                    .map_err(|e| anyhow::anyhow!("Konfigurationsfehler in '{pfad}': {e}"))?;
                Ok(config)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                tracing::warn!(
                    pfad = pfad,
                    "Konfigurationsdatei nicht gefunden, verwende Standardwerte"
                );
                Ok(Self::default())
            }
            Err(e) => Err(anyhow::anyhow!(
                "Konfigurationsdatei '{pfad}' nicht lesbar: {e}"
            )),
        }
    }

    /// Gibt die vollstaendige Bind-Adresse fuer TCP zurueck
    pub fn tcp_bind_adresse(&self) -> String {
        format!("{}:{}", self.netzwerk.bind_adresse, self.netzwerk.tcp_port)
    }

    /// Gibt die vollstaendige Bind-Adresse fuer UDP zurueck
    pub fn udp_bind_adresse(&self) -> String {
        format!("{}:{}", self.netzwerk.bind_adresse, self.netzwerk.udp_port)
    }

    /// Gibt die Bind-Adresse fuer den Commander REST-Server zurueck
    pub fn commander_rest_bind_adresse(&self) -> String {
        format!(
            "{}:{}",
            self.netzwerk.bind_adresse, self.commander.rest_port
        )
    }

    /// Gibt die Bind-Adresse fuer den Commander TCP/TLS-Server zurueck
    pub fn commander_tcp_bind_adresse(&self) -> String {
        format!("{}:{}", self.netzwerk.bind_adresse, self.commander.tcp_port)
    }

    /// Gibt die Bind-Adresse fuer den gRPC-Server zurueck
    pub fn grpc_bind_adresse(&self) -> String {
        format!("{}:{}", self.netzwerk.bind_adresse, self.netzwerk.grpc_port)
    }

    /// Gibt die Bind-Adresse fuer den Observability-Server zurueck
    pub fn observability_bind_adresse(&self) -> String {
        format!("{}:{}", self.netzwerk.bind_adresse, self.observability.port)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_config_ist_valide() {
        let cfg = ServerConfig::default();
        assert_eq!(cfg.server.max_clients, 512);
        assert_eq!(cfg.netzwerk.tcp_port, 9987);
        assert_eq!(cfg.datenbank.typ, "sqlite");
        assert_eq!(cfg.logging.level, "info");
    }

    #[test]
    fn bind_adressen() {
        let cfg = ServerConfig::default();
        assert_eq!(cfg.tcp_bind_adresse(), "0.0.0.0:9987");
        assert_eq!(cfg.udp_bind_adresse(), "0.0.0.0:9987");
    }

    #[test]
    fn config_aus_toml_string() {
        let toml = r#"
            [server]
            name = "Mein Server"
            max_clients = 100

            [netzwerk]
            tcp_port = 10000
        "#;
        let cfg: ServerConfig = toml::from_str(toml).unwrap();
        assert_eq!(cfg.server.name, "Mein Server");
        assert_eq!(cfg.server.max_clients, 100);
        assert_eq!(cfg.netzwerk.tcp_port, 10000);
        // Nicht angegebene Felder behalten Standardwerte
        assert_eq!(cfg.netzwerk.udp_port, 9987);
    }
}
