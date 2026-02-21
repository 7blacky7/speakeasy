//! speakeasy-server – Bibliotheks-Root
//!
//! Deklariert alle Server-Module und stellt den oeffentlichen Einstiegspunkt
//! fuer Integrationstests bereit.

pub mod config;

use anyhow::Result;
use config::ServerConfig;

/// Haelt den laufenden Server-Zustand zusammen
pub struct Server {
    pub config: ServerConfig,
}

impl Server {
    /// Erstellt einen neuen Server aus der gegebenen Konfiguration
    pub fn neu(config: ServerConfig) -> Self {
        Self { config }
    }

    /// Startet alle Server-Subsysteme und laeuft bis zum Shutdown-Signal
    ///
    /// Reihenfolge:
    /// 1. Datenbankverbindung herstellen
    /// 2. TCP/TLS-Listener starten (Control-Protokoll)
    /// 3. UDP-Socket oeffnen (Voice)
    /// 4. REST-API starten
    /// 5. Auf Ctrl-C / SIGTERM warten
    pub async fn starten(self) -> Result<()> {
        tracing::info!(
            server_name = %self.config.server.name,
            tcp = %self.config.tcp_bind_adresse(),
            udp = %self.config.udp_bind_adresse(),
            api_port = self.config.netzwerk.api_port,
            "Server startet"
        );

        tracing::info!(
            backend = %self.config.datenbank.typ,
            url = %self.config.datenbank.url,
            "Datenbankverbindung wird hergestellt (Platzhalter)"
        );

        tracing::info!(
            adresse = %self.config.tcp_bind_adresse(),
            "TCP-Listener bereit (Platzhalter)"
        );

        tracing::info!(
            adresse = %self.config.udp_bind_adresse(),
            "UDP-Socket bereit (Platzhalter)"
        );

        tracing::info!(
            port = self.config.netzwerk.api_port,
            "REST-API bereit (Platzhalter)"
        );

        tracing::info!("Server laeuft. Warte auf Shutdown-Signal (Ctrl-C)...");
        tokio::signal::ctrl_c().await?;
        tracing::info!("Shutdown-Signal empfangen, Server wird beendet");

        Ok(())
    }
}
