//! Client-Connection – Verwaltet eine einzelne TCP-Verbindung
//!
//! Jede TCP-Verbindung bekommt eine `ClientConnection` in einem eigenen
//! tokio-Task. Die State Machine verwaltet den Verbindungszustand.
//!
//! ## State Machine
//! ```text
//! Connected -> Authenticating -> Authenticated -> InChannel
//!     ^                              |
//!     |                             v
//!     +------- Disconnect ----------+
//! ```
//!
//! ## Keepalive
//! - Server sendet alle `keepalive_sek` einen Ping
//! - Client muss innerhalb von `verbindungs_timeout_sek` antworten
//! - Bei Timeout wird die Verbindung getrennt

use futures_util::{SinkExt, StreamExt};
use speakeasy_db::{
    repository::UserRepository, BanRepository, ChannelRepository, ChatMessageRepository,
    PermissionRepository, ServerGroupRepository,
};
use speakeasy_protocol::{
    control::{ControlMessage, ErrorCode},
    wire::FrameCodec,
};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_util::codec::Framed;

use crate::dispatcher::{DispatcherContext, MessageDispatcher};
use crate::server_state::SignalingState;

// ---------------------------------------------------------------------------
// Verbindungszustand
// ---------------------------------------------------------------------------

/// Zustand der TCP-Verbindung
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerbindungsZustand {
    /// Verbunden, noch nicht authentifiziert
    Verbunden,
    /// Authentifizierung laeuft
    Authentifizierung,
    /// Erfolgreich authentifiziert
    Authentifiziert,
    /// In einem Voice-Channel
    ImChannel,
    /// Verbindung wird getrennt
    Trennend,
}

// ---------------------------------------------------------------------------
// ClientConnection
// ---------------------------------------------------------------------------

/// Verarbeitet eine einzelne TCP-Verbindung
///
/// Liest Frames via `FrameCodec`, dispatcht an `MessageDispatcher` und
/// sendet Antworten zurueck. Luft in einem eigenen tokio-Task.
pub struct ClientConnection<U, P, B>
where
    U: UserRepository + ServerGroupRepository + ChannelRepository + ChatMessageRepository + 'static,
    P: PermissionRepository + 'static,
    B: BanRepository + 'static,
{
    state: Arc<SignalingState<U, P, B>>,
    peer_addr: SocketAddr,
}

impl<U, P, B> ClientConnection<U, P, B>
where
    U: UserRepository + ServerGroupRepository + ChannelRepository + ChatMessageRepository + 'static,
    P: PermissionRepository + 'static,
    B: BanRepository + 'static,
{
    /// Erstellt eine neue ClientConnection
    pub fn neu(state: Arc<SignalingState<U, P, B>>, peer_addr: SocketAddr) -> Self {
        Self { state, peer_addr }
    }

    /// Startet die Verbindungs-Verarbeitungsschleife
    ///
    /// Diese Methode laeuft bis die Verbindung getrennt wird oder ein
    /// Shutdown-Signal eingeht.
    pub async fn verarbeiten(
        self,
        stream: TcpStream,
        mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
    ) {
        let peer_addr = self.peer_addr;
        let keepalive_intervall = Duration::from_secs(self.state.config.keepalive_sek);
        let timeout_dauer = Duration::from_secs(self.state.config.verbindungs_timeout_sek);

        tracing::info!(peer = %peer_addr, "Neue Verbindung");

        // Framed-Stream mit FrameCodec einrichten
        let mut framed = Framed::new(stream, FrameCodec::new());

        // Ausgehende Nachrichten-Queue (Broadcaster -> TCP)
        // Wird nach dem Login mit der Broadcaster-Queue des Users verknuepft
        let (sende_tx, mut sende_rx) = mpsc::channel::<ControlMessage>(64);

        // Dispatcher und Kontext initialisieren
        let (shutdown_watch_tx, _) = tokio::sync::watch::channel(false);
        let mut ctx = DispatcherContext {
            peer_addr,
            session_token: None,
            user_id: None,
            shutdown_tx: shutdown_watch_tx,
        };
        let dispatcher = MessageDispatcher::neu(Arc::clone(&self.state));

        // Zeitpunkt des letzten empfangenen Frames
        let mut letzter_empfang = Instant::now();
        // Zeitpunkt des naechsten Ping
        let mut naechster_ping = Instant::now() + keepalive_intervall;
        let mut ping_request_id: u32 = 0;

        loop {
            let jetzt = Instant::now();

            // Timeout-Pruefung
            if jetzt.duration_since(letzter_empfang) > timeout_dauer {
                tracing::warn!(peer = %peer_addr, "Verbindungs-Timeout");
                break;
            }

            // Naechsten Ping-Zeitpunkt berechnen
            let ping_verzoegerung = if jetzt < naechster_ping {
                naechster_ping.duration_since(jetzt)
            } else {
                Duration::from_millis(1)
            };

            tokio::select! {
                // Eingehende Nachricht vom Client
                frame = framed.next() => {
                    match frame {
                        Some(Ok(nachricht)) => {
                            letzter_empfang = Instant::now();
                            tracing::trace!(
                                peer = %peer_addr,
                                request_id = nachricht.request_id,
                                "Nachricht empfangen"
                            );

                            // Nach Login: Broadcaster-Queue verbinden
                            if ctx.user_id.is_some() {
                                // Broadcast-Queue wird beim ersten authentifizierten
                                // Request ueber den Broadcaster registriert
                            }

                            // Dispatch
                            if let Some(antwort) = dispatcher.dispatch(nachricht, &mut ctx).await {
                                if let Err(e) = framed.send(antwort).await {
                                    tracing::warn!(
                                        peer = %peer_addr,
                                        fehler = %e,
                                        "Senden fehlgeschlagen"
                                    );
                                    break;
                                }
                            }

                            // Nach erfolgreichem Login: Broadcaster-Queue abonnieren
                            if let Some(uid) = ctx.user_id {
                                if !self.state.broadcaster.ist_registriert(&uid) {
                                    let mut recv_queue =
                                        self.state.broadcaster.client_registrieren(uid);
                                    // Spawn separaten Lese-Task fuer Broadcast-Queue
                                    let sende_tx_clone = sende_tx.clone();
                                    tokio::spawn(async move {
                                        while let Some(msg) = recv_queue.recv().await {
                                            if sende_tx_clone.send(msg).await.is_err() {
                                                break;
                                            }
                                        }
                                    });
                                }
                            }
                        }
                        Some(Err(e)) => {
                            tracing::warn!(
                                peer = %peer_addr,
                                fehler = %e,
                                "Frame-Lesefehler"
                            );
                            break;
                        }
                        None => {
                            // Verbindung geschlossen
                            tracing::info!(peer = %peer_addr, "Verbindung vom Client getrennt");
                            break;
                        }
                    }
                }

                // Ausgehende Nachricht aus dem Broadcaster
                Some(ausgehend) = sende_rx.recv() => {
                    if let Err(e) = framed.send(ausgehend).await {
                        tracing::warn!(
                            peer = %peer_addr,
                            fehler = %e,
                            "Broadcast-Senden fehlgeschlagen"
                        );
                        break;
                    }
                }

                // Keepalive-Ping
                _ = tokio::time::sleep(ping_verzoegerung) => {
                    if jetzt >= naechster_ping {
                        ping_request_id = ping_request_id.wrapping_add(1);
                        let ts = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64;
                        let ping = ControlMessage::ping(ping_request_id, ts);

                        if let Err(e) = framed.send(ping).await {
                            tracing::warn!(
                                peer = %peer_addr,
                                fehler = %e,
                                "Ping-Senden fehlgeschlagen"
                            );
                            break;
                        }
                        naechster_ping = Instant::now() + keepalive_intervall;
                    }
                }

                // Shutdown-Signal
                Ok(()) = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        tracing::info!(peer = %peer_addr, "Shutdown-Signal – Verbindung wird getrennt");
                        // Abschiedsnachricht senden
                        let abschied = ControlMessage::error(
                            0,
                            ErrorCode::InternalError,
                            "Server wird heruntergefahren",
                        );
                        let _ = framed.send(abschied).await;
                        break;
                    }
                }
            }
        }

        // Cleanup beim Verbindungsende
        if let Some(uid) = ctx.user_id {
            dispatcher.client_cleanup(&uid).await;

            // Session invalidieren
            if let Some(ref token) = ctx.session_token {
                let _ = self.state.auth_service.abmelden(token).await;
            }
        }

        tracing::info!(peer = %peer_addr, "Verbindungs-Task beendet");
    }
}
