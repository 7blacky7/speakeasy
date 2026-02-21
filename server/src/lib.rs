//! speakeasy-server – Bibliotheks-Root
//!
//! Deklariert alle Server-Module und stellt den oeffentlichen Einstiegspunkt
//! fuer Integrationstests bereit.

pub mod config;

use std::sync::Arc;

use anyhow::Result;
use config::ServerConfig;

use speakeasy_auth::{ApiTokenStore, AuthService, BanService, PermissionService, SessionStore};
use speakeasy_db::{
    repository::{DatabaseBackend, DatabaseConfig, UserRepository},
    SqliteDb,
};

/// Standard-Passwort fuer den Admin-Benutzer beim ersten Start
const ADMIN_STANDARD_PASSWORT: &str = "admin";
/// Standard-Benutzername fuer den Admin
const ADMIN_BENUTZERNAME: &str = "admin";

/// Gemeinsamer Zustand des Servers (thread-safe, via Arc geteilt)
///
/// Alle Services werden als Arc gehalten und koennen sicher zwischen
/// Tokio-Tasks geteilt werden.
pub struct ServerState {
    pub auth_service: Arc<AuthService<SqliteDb>>,
    pub permission_service: Arc<PermissionService<SqliteDb>>,
    pub ban_service: Arc<BanService<SqliteDb>>,
}

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
    /// 1. Datenbankverbindung herstellen und Migrationen ausfuehren
    /// 2. Auth-, Permission- und Ban-Services initialisieren
    /// 3. Erster Start: Admin-Benutzer anlegen wenn keine Benutzer vorhanden
    /// 4. TCP/TLS-Listener starten (Control-Protokoll)
    /// 5. UDP-Socket oeffnen (Voice)
    /// 6. REST-API starten
    /// 7. Auf Ctrl-C / SIGTERM warten
    pub async fn starten(self) -> Result<()> {
        tracing::info!(
            server_name = %self.config.server.name,
            tcp = %self.config.tcp_bind_adresse(),
            udp = %self.config.udp_bind_adresse(),
            api_port = self.config.netzwerk.api_port,
            "Server startet"
        );

        // --- 1. Datenbankverbindung ---
        let db_config = DatabaseConfig {
            backend: DatabaseBackend::Sqlite,
            url: self.config.datenbank.url.clone(),
            max_verbindungen: self.config.datenbank.max_verbindungen,
            sqlite_wal: true,
        };

        tracing::info!(
            backend = %db_config.backend,
            url = %db_config.url,
            "Datenbankverbindung wird hergestellt"
        );

        let db = Arc::new(
            SqliteDb::oeffnen(&db_config)
                .await
                .map_err(|e| anyhow::anyhow!("Datenbankverbindung fehlgeschlagen: {e}"))?,
        );

        tracing::info!("Datenbankverbindung hergestellt, Migrationen ausgefuehrt");

        // --- 2. Services initialisieren ---
        let session_store = SessionStore::neu();
        let session_store = SessionStore::neu_mit_cleanup(session_store);

        let api_token_store = ApiTokenStore::neu();

        let auth_service = Arc::new(AuthService::neu(
            Arc::clone(&db),
            Arc::clone(&session_store),
            Arc::clone(&api_token_store),
        ));

        let permission_service = PermissionService::neu(Arc::clone(&db));

        let ban_service = BanService::neu(Arc::clone(&db));

        tracing::info!("Auth-, Permission- und Ban-Services initialisiert");

        // --- 3. Erster Start: Admin-Benutzer anlegen ---
        ersten_start_initialisieren(&db, &auth_service).await?;

        // Den ServerState zusammenbauen (wird spaeter fuer Handler benoetigt)
        let _state = Arc::new(ServerState {
            auth_service,
            permission_service,
            ban_service,
        });

        // --- 4-6. Netzwerk-Listener (Platzhalter) ---
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

/// Prueft beim ersten Start ob Benutzer vorhanden sind.
/// Wenn nicht, wird ein Admin-Benutzer mit Standardpasswort angelegt.
async fn ersten_start_initialisieren(
    db: &SqliteDb,
    auth_service: &AuthService<SqliteDb>,
) -> Result<()> {
    let alle_benutzer = db
        .list(false)
        .await
        .map_err(|e| anyhow::anyhow!("Benutzer-Abfrage fehlgeschlagen: {e}"))?;

    if alle_benutzer.is_empty() {
        tracing::warn!(
            "Erster Start erkannt: Kein Benutzer vorhanden. Admin-Benutzer wird angelegt."
        );

        match auth_service
            .registrieren(ADMIN_BENUTZERNAME, ADMIN_STANDARD_PASSWORT)
            .await
        {
            Ok(admin) => {
                tracing::warn!(
                    user_id = %admin.id,
                    username = %admin.username,
                    "Admin-Benutzer angelegt. BITTE PASSWORT SOFORT AENDERN! \
                     Standardpasswort: '{}'",
                    ADMIN_STANDARD_PASSWORT
                );
            }
            Err(speakeasy_auth::AuthError::BenutzernameVergeben(_)) => {
                tracing::info!("Admin-Benutzer existiert bereits");
            }
            Err(e) => {
                return Err(anyhow::anyhow!("Admin-Erstellung fehlgeschlagen: {e}"));
            }
        }
    } else {
        tracing::debug!(
            anzahl_benutzer = alle_benutzer.len(),
            "Bestehende Installation erkannt, kein erster Start"
        );
    }

    Ok(())
}
