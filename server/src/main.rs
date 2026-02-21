//! Speakeasy Server – Einstiegspunkt
//!
//! Laedt die Konfiguration, initialisiert das Logging und startet den Server.

use anyhow::Result;
use speakeasy_server::{Server, config::ServerConfig};

#[tokio::main]
async fn main() -> Result<()> {
    // Konfigurationsdatei-Pfad aus Umgebungsvariable oder Standard
    let config_pfad = std::env::var("SPEAKEASY_CONFIG")
        .unwrap_or_else(|_| "config.toml".into());

    // Konfiguration laden (Standardwerte falls Datei fehlt)
    let config = ServerConfig::laden(&config_pfad)?;

    // Logging initialisieren
    logging_initialisieren(&config.logging.level, &config.logging.format);

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        config = %config_pfad,
        "Speakeasy Server wird initialisiert"
    );

    // Server starten
    let server = Server::neu(config);
    server.starten().await?;

    Ok(())
}

/// Initialisiert tracing-subscriber mit dem konfigurierten Level und Format
fn logging_initialisieren(level: &str, format: &str) {
    use tracing_subscriber::{EnvFilter, fmt};

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(level));

    match format {
        "json" => {
            fmt()
                .json()
                .with_env_filter(filter)
                .with_target(true)
                .with_thread_ids(true)
                .init();
        }
        _ => {
            fmt()
                .with_env_filter(filter)
                .with_target(true)
                .init();
        }
    }
}
