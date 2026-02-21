//! TCP-Listener – Bindet Socket, akzeptiert Verbindungen
//!
//! Der `SignalingServer` bindet einen TCP-Socket und startet fuer jede
//! eingehende Verbindung einen eigenen tokio-Task mit einer `ClientConnection`.
//!
//! ## Concurrency-Modell
//! Da die Repository-Traits async fn ohne Send-Garantie verwenden
//! (async_fn_in_trait), laufen alle Verbindungs-Tasks in einer
//! `tokio::task::LocalSet` auf einem single-threaded Executor.
//! Dies ist korrekt fuer einen einzelnen Server-Prozess.

use speakeasy_db::{
    repository::UserRepository, BanRepository, ChannelRepository, ChatMessageRepository,
    PermissionRepository, ServerGroupRepository,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::task::LocalSet;

use crate::connection::ClientConnection;
use crate::server_state::SignalingState;

/// TCP-Signaling-Server
///
/// Bindet einen TCP-Socket und akzeptiert Verbindungen in einer Loop.
/// Jede Verbindung wird als lokaler Task in der `LocalSet` ausgefuehrt.
pub struct SignalingServer<U, P, B>
where
    U: UserRepository + ServerGroupRepository + ChannelRepository + ChatMessageRepository + 'static,
    P: PermissionRepository + 'static,
    B: BanRepository + 'static,
{
    state: Arc<SignalingState<U, P, B>>,
    bind_addr: SocketAddr,
}

impl<U, P, B> SignalingServer<U, P, B>
where
    U: UserRepository + ServerGroupRepository + ChannelRepository + ChatMessageRepository + 'static,
    P: PermissionRepository + 'static,
    B: BanRepository + 'static,
{
    /// Erstellt einen neuen SignalingServer
    pub fn neu(state: Arc<SignalingState<U, P, B>>, bind_addr: SocketAddr) -> Self {
        Self { state, bind_addr }
    }

    /// Startet den TCP-Listener und akzeptiert Verbindungen
    ///
    /// Laeuft bis `shutdown_rx` ein `true`-Signal empfaengt.
    /// Verwendet eine `LocalSet` fuer alle Verbindungs-Tasks.
    pub async fn starten(
        self,
        shutdown_rx: tokio::sync::watch::Receiver<bool>,
    ) -> std::io::Result<()> {
        let local = LocalSet::new();
        local.run_until(self.accept_loop(shutdown_rx)).await
    }

    /// Interne Accept-Loop (laeuft innerhalb der LocalSet)
    async fn accept_loop(
        self,
        mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
    ) -> std::io::Result<()> {
        let listener = TcpListener::bind(self.bind_addr).await?;
        let lokale_addr = listener.local_addr()?;

        tracing::info!(
            adresse = %lokale_addr,
            "TCP Signaling-Server gestartet"
        );

        loop {
            tokio::select! {
                // Neue eingehende Verbindung
                result = listener.accept() => {
                    match result {
                        Ok((stream, peer_addr)) => {
                            // Client-Limit pruefen
                            let online = self.state.presence.online_anzahl() as u32;
                            if online >= self.state.config.max_clients {
                                tracing::warn!(
                                    peer = %peer_addr,
                                    max = self.state.config.max_clients,
                                    "Server voll – Verbindung abgelehnt"
                                );
                                drop(stream);
                                continue;
                            }

                            tracing::debug!(peer = %peer_addr, "Verbindung akzeptiert");

                            let verbindung = ClientConnection::neu(
                                Arc::clone(&self.state),
                                peer_addr,
                            );
                            let shutdown_rx_clone = shutdown_rx.clone();

                            // Lokaler Task – kein Send erforderlich
                            tokio::task::spawn_local(async move {
                                verbindung.verarbeiten(stream, shutdown_rx_clone).await;
                            });
                        }
                        Err(e) => {
                            tracing::error!(fehler = %e, "TCP-Accept-Fehler");
                            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                        }
                    }
                }

                // Shutdown-Signal
                Ok(()) = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        tracing::info!("Signaling-Server: Shutdown-Signal empfangen");
                        break;
                    }
                }
            }
        }

        tracing::info!("TCP Signaling-Server gestoppt");
        Ok(())
    }

    /// Gibt die Bind-Adresse zurueck
    pub fn bind_addr(&self) -> SocketAddr {
        self.bind_addr
    }
}
