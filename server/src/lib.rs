//! speakeasy-server – Bibliotheks-Root
//!
//! Deklariert alle Server-Module und stellt den oeffentlichen Einstiegspunkt
//! fuer Integrationstests bereit.

pub mod config;

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use config::ServerConfig;

use speakeasy_auth::{ApiTokenStore, AuthService, BanService, PermissionService, SessionStore};
use speakeasy_commander::rest::{CommanderState, ExecutorFn, TokenValidatorFn};
use speakeasy_commander::{CommandExecutor, RateLimitKonfig, RateLimiter};
use speakeasy_db::{
    repository::{DatabaseBackend, DatabaseConfig, UserRepository},
    SqliteDb,
};
use speakeasy_plugin::{ManagerKonfiguration, PluginManager};
use speakeasy_signaling::{server_state::SignalingConfig, SignalingServer};
use speakeasy_voice::udp::{VoiceServer, VoiceServerConfig};
use speakeasy_voice::{ChannelRouter, VoiceState};

/// Standard-Passwort fuer den Admin-Benutzer beim ersten Start
const ADMIN_STANDARD_PASSWORT: &str = "admin";
/// Standard-Benutzername fuer den Admin
const ADMIN_BENUTZERNAME: &str = "admin";

/// Gemeinsamer Zustand des Servers (thread-safe, via Arc geteilt)
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
    /// 4. Chat-Service erstellen
    /// 5. Voice-Server starten (UDP)
    /// 6. Signaling-Server starten (TCP) – eigener Thread mit LocalSet
    /// 7. Commander starten (REST + gRPC)
    /// 8. Observability starten (Metriken + Health)
    /// 9. Plugin-Manager initialisieren
    /// 10. Auf Ctrl-C warten und Graceful Shutdown
    pub async fn starten(self) -> Result<()> {
        tracing::info!(
            server_name = %self.config.server.name,
            tcp = %self.config.tcp_bind_adresse(),
            udp = %self.config.udp_bind_adresse(),
            rest_port = self.config.commander.rest_port,
            grpc_port = self.config.netzwerk.grpc_port,
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

        // --- 2. Auth-, Permission- und Ban-Services ---
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

        let _state = Arc::new(ServerState {
            auth_service: Arc::clone(&auth_service),
            permission_service: Arc::clone(&permission_service),
            ban_service: Arc::clone(&ban_service),
        });

        // --- 4. Chat-Service ---
        let chat_service = speakeasy_chat::ChatService::neu(Arc::clone(&db));
        let _file_storage = Arc::new(speakeasy_chat::DiskStorage::new("data/files"));
        tracing::info!("Chat-Service initialisiert");

        // --- 5. Voice-Server starten (UDP) ---
        let udp_addr: SocketAddr = self.config.udp_bind_adresse().parse()?;
        let voice_router = ChannelRouter::neu();
        let voice_state = VoiceState::neu();
        let voice_config = VoiceServerConfig::neu(udp_addr);

        let voice_server =
            VoiceServer::binden(voice_config, voice_router.clone(), voice_state.clone())
                .await
                .map_err(|e| anyhow::anyhow!("Voice-Server konnte nicht binden: {e}"))?;

        let voice_server = Arc::new(voice_server);
        let (voice_shutdown_tx, voice_shutdown_rx) = tokio::sync::oneshot::channel::<()>();

        let voice_clone = Arc::clone(&voice_server);
        let voice_handle = tokio::spawn(async move {
            voice_clone.empfangs_loop_starten(voice_shutdown_rx).await;
        });

        tracing::info!(
            adresse = %udp_addr,
            "Voice-Server gestartet (UDP)"
        );

        // --- 6. Signaling-Server starten (TCP) ---
        // SignalingServer nutzt LocalSet wegen async_fn_in_trait ohne Send.
        // Deshalb starten wir ihn in einem eigenen Thread mit current_thread Runtime.
        // Krypto-Modus und DTLS-Fingerprint bestimmen
        let (crypto_mode, dtls_fingerprint) = if self.config.netzwerk.tls_zertifikat.is_some() {
            // TLS konfiguriert -> DTLS-Modus
            match speakeasy_crypto::generate_self_signed_cert("speakeasy-server") {
                Ok(dtls_config) => {
                    tracing::info!(
                        fingerprint = %dtls_config.certificate_fingerprint,
                        "DTLS-Zertifikat generiert"
                    );
                    (
                        "dtls".to_string(),
                        Some(dtls_config.certificate_fingerprint),
                    )
                }
                Err(e) => {
                    tracing::warn!(fehler = %e, "DTLS-Zertifikat konnte nicht generiert werden, fallback auf none");
                    ("none".to_string(), None)
                }
            }
        } else {
            ("none".to_string(), None)
        };

        let signaling_config = SignalingConfig {
            server_name: self.config.server.name.clone(),
            welcome_message: self.config.server.willkommen.clone(),
            max_clients: self.config.server.max_clients,
            voice_udp_port: self.config.netzwerk.udp_port,
            voice_server_ip: self.config.netzwerk.bind_adresse.clone(),
            crypto_mode,
            dtls_fingerprint,
            ..Default::default()
        };

        let tcp_addr: SocketAddr = self.config.tcp_bind_adresse().parse()?;
        let (signaling_shutdown_tx, signaling_shutdown_rx) = tokio::sync::watch::channel(false);

        let signaling_state = speakeasy_signaling::server_state::SignalingState::neu(
            signaling_config,
            Arc::clone(&auth_service),
            Arc::clone(&permission_service),
            Arc::clone(&ban_service),
            Arc::clone(&db),
            Arc::clone(&chat_service),
        );

        let signaling_server = SignalingServer::neu(signaling_state, tcp_addr);

        // Eigener Thread fuer LocalSet (nicht-Send Futures)
        let signaling_handle = std::thread::Builder::new()
            .name("signaling".into())
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("Signaling-Runtime konnte nicht erstellt werden");
                rt.block_on(async move {
                    if let Err(e) = signaling_server.starten(signaling_shutdown_rx).await {
                        tracing::error!(fehler = %e, "Signaling-Server Fehler");
                    }
                });
            })
            .map_err(|e| anyhow::anyhow!("Signaling-Thread konnte nicht gestartet werden: {e}"))?;

        tracing::info!(
            adresse = %tcp_addr,
            "Signaling-Server gestartet (TCP)"
        );

        // --- 7. Commander starten (REST + gRPC) ---
        let commander_executor = CommandExecutor::neu(
            Arc::clone(&db), // user_repo
            Arc::clone(&db), // channel_repo
            Arc::clone(&db), // permission_repo
            Arc::clone(&db), // ban_repo
            Arc::clone(&db), // audit_repo
            Arc::clone(&auth_service),
            Arc::clone(&permission_service),
            Arc::clone(&ban_service),
            self.config.server.name.clone(),
            env!("CARGO_PKG_VERSION").to_string(),
        );

        // Type-erased executor closure fuer CommanderState
        let executor_arc = Arc::clone(&commander_executor);
        let executor_fn: ExecutorFn = Arc::new(move |cmd, session| {
            let exec = Arc::clone(&executor_arc);
            Box::pin(async move { exec.ausfuehren(cmd, &session).await })
        });

        // Type-erased token validator (synchron, nutzt block_in_place fuer async SessionStore)
        let commander_auth = Arc::new(speakeasy_commander::auth::CommanderAuth::neu(Arc::clone(
            &auth_service,
        )));
        let token_validator: TokenValidatorFn = Arc::new(move |token: &str| {
            let auth = Arc::clone(&commander_auth);
            let token = token.to_string();
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current()
                    .block_on(async { auth.token_validieren(&token).await })
            })
        });

        let commander_state = CommanderState::neu(executor_fn, token_validator);

        // REST-Server
        let rest_addr: SocketAddr = self.config.commander_rest_bind_adresse().parse()?;
        let rest_konfig = speakeasy_commander::rest::server::RestServerKonfig {
            bind_addr: rest_addr,
            cors_origins: self.config.commander.cors_origins.clone(),
        };
        let rate_limiter = RateLimiter::neu(RateLimitKonfig::default());

        let rest_state = commander_state.clone();
        let rest_limiter = Arc::clone(&rate_limiter);
        let rest_handle = tokio::spawn(async move {
            let server = speakeasy_commander::rest::RestServer::neu(rest_konfig);
            if let Err(e) = server.starten(rest_state, rest_limiter).await {
                tracing::error!(fehler = %e, "REST-Commander-Server Fehler");
            }
        });

        tracing::info!(
            adresse = %rest_addr,
            "Commander REST-Server gestartet"
        );

        // gRPC-Server
        let grpc_addr: SocketAddr = self.config.grpc_bind_adresse().parse()?;
        let grpc_konfig = speakeasy_commander::grpc::GrpcServerKonfig {
            bind_addr: grpc_addr,
        };

        let grpc_state = commander_state.clone();
        let grpc_handle = tokio::spawn(async move {
            let server = speakeasy_commander::grpc::GrpcServer::neu(grpc_konfig);
            if let Err(e) = server.starten(grpc_state).await {
                tracing::error!(fehler = %e, "gRPC-Commander-Server Fehler");
            }
        });

        tracing::info!(
            adresse = %grpc_addr,
            "Commander gRPC-Server gestartet"
        );

        // --- 8. Observability starten ---
        let obs_handle = if self.config.observability.aktiviert {
            let obs_addr: SocketAddr = self.config.observability_bind_adresse().parse()?;
            let handle = tokio::spawn(async move {
                if let Err(e) =
                    speakeasy_observability::observability_server_starten(obs_addr).await
                {
                    tracing::error!(fehler = %e, "Observability-Server Fehler");
                }
            });
            tracing::info!(
                adresse = %self.config.observability_bind_adresse(),
                "Observability-Server gestartet (Metriken + Health)"
            );
            Some(handle)
        } else {
            tracing::info!("Observability deaktiviert");
            None
        };

        // --- 9. Plugin-Manager ---
        let _plugin_manager = if self.config.plugins.aktiviert {
            let manager = PluginManager::neu(ManagerKonfiguration::default());
            tracing::info!(
                verzeichnis = ?self.config.plugins.verzeichnis,
                "Plugin-Manager initialisiert"
            );
            Some(manager)
        } else {
            tracing::info!("Plugin-System deaktiviert");
            None
        };

        // --- 10. Warten auf Shutdown-Signal ---
        tracing::info!(
            "Server laeuft. Alle Subsysteme gestartet. Warte auf Shutdown-Signal (Ctrl-C)..."
        );
        tokio::signal::ctrl_c().await?;
        tracing::info!("Shutdown-Signal empfangen, fahre Server herunter...");

        // Graceful Shutdown aller Services
        // Voice-Server stoppen
        let _ = voice_shutdown_tx.send(());
        tracing::debug!("Voice-Server Shutdown-Signal gesendet");

        // Signaling-Server stoppen
        let _ = signaling_shutdown_tx.send(true);
        tracing::debug!("Signaling-Server Shutdown-Signal gesendet");

        // Commander-Tasks abbrechen (keine graceful shutdown API)
        rest_handle.abort();
        grpc_handle.abort();
        tracing::debug!("Commander-Server gestoppt");

        // Observability stoppen
        if let Some(handle) = obs_handle {
            handle.abort();
            tracing::debug!("Observability-Server gestoppt");
        }

        // Voice-Task abwarten
        let _ = voice_handle.await;
        tracing::debug!("Voice-Server beendet");

        // Signaling-Thread abwarten
        if let Err(e) = signaling_handle.join() {
            tracing::warn!("Signaling-Thread Fehler beim Beenden: {:?}", e);
        }
        tracing::debug!("Signaling-Server beendet");

        tracing::info!("Server erfolgreich heruntergefahren");

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
