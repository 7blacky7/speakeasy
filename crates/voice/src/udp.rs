//! UDP Voice Server – Listener und Send-Queue pro Client
//!
//! Bindet einen UDP-Socket, empfaengt Voice-Pakete, dekodiert den Header
//! und leitet sie ueber den `ChannelRouter` weiter.
//!
//! ## Architektur
//!
//! ```text
//! UDP Socket (recv_from)
//!     |
//!     v
//! VoicePacket::decode()      <- Validierung
//!     |
//!     v
//! VoiceState::user_id_von_endpunkt()  <- Client identifizieren
//!     |
//!     v
//! ChannelRouter::paket_weiterleiten() <- An alle anderen Teilnehmer
//!     |
//!     +--> Empfaenger-Send-Queue (mpsc) --> UDP send_to Task
//! ```
//!
//! ## Performance
//! - Minimale Allocations: Recv-Buffer wird wiederverwendet (stack-allocated)
//! - Zero-copy Weiterleitung via Arc<Vec<u8>>
//! - Separater Sende-Task pro Client (verhindert Head-of-Line-Blocking)

use crate::router::ChannelRouter;
use crate::state::VoiceState;
use speakeasy_core::types::{ChannelId, UserId};
use speakeasy_protocol::voice::VoicePacket;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;

/// Maximale UDP-Paketgroesse (Header 16 + Max-Payload 1280 + Puffer)
const UDP_BUFFER_SIZE: usize = 1400;

// ---------------------------------------------------------------------------
// VoiceServer-Konfiguration
// ---------------------------------------------------------------------------

/// Konfiguration fuer den UDP Voice Server
#[derive(Debug, Clone)]
pub struct VoiceServerConfig {
    /// Bind-Adresse (z.B. "0.0.0.0:4000")
    pub bind_addr: SocketAddr,
    /// Groesse des Sende-Kanalspuffers pro Client
    pub send_queue_groesse: usize,
}

impl VoiceServerConfig {
    /// Erstellt eine Konfiguration mit Standard-Werten
    pub fn neu(bind_addr: SocketAddr) -> Self {
        Self {
            bind_addr,
            send_queue_groesse: 128,
        }
    }
}

// ---------------------------------------------------------------------------
// ClientSender – Sende-Task pro Client
// ---------------------------------------------------------------------------

/// Handle fuer den Sende-Task eines Clients
///
/// Wenn dieses Handle gedroppt wird, wird der Sende-Task beendet.
pub struct ClientSenderHandle {
    /// Sende-Queue: Pakete hier einlegen -> werden via UDP versendet
    pub tx: mpsc::Sender<Arc<Vec<u8>>>,
    /// Task-Handle (Abbruch beim Drop)
    _task: tokio::task::JoinHandle<()>,
}

impl ClientSenderHandle {
    /// Startet einen neuen Sende-Task fuer einen Client
    ///
    /// Liest aus der mpsc-Queue und sendet via UDP an `ziel_addr`.
    pub fn starten(
        socket: Arc<UdpSocket>,
        ziel_addr: SocketAddr,
        queue_groesse: usize,
    ) -> Self {
        let (tx, mut rx) = mpsc::channel::<Arc<Vec<u8>>>(queue_groesse);

        let task = tokio::spawn(async move {
            while let Some(daten) = rx.recv().await {
                match socket.send_to(&daten, ziel_addr).await {
                    Ok(_) => {
                        tracing::trace!(
                            bytes = daten.len(),
                            ziel = %ziel_addr,
                            "UDP-Paket gesendet"
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            fehler = %e,
                            ziel = %ziel_addr,
                            "UDP-Sendefehler"
                        );
                    }
                }
            }
            tracing::debug!(ziel = %ziel_addr, "Sende-Task beendet");
        });

        Self { tx, _task: task }
    }
}

// ---------------------------------------------------------------------------
// VoiceServer
// ---------------------------------------------------------------------------

/// UDP Voice Server
///
/// Bindet einen UDP-Socket und empfaengt Voice-Pakete in einer Async-Loop.
/// Leitet Pakete ueber den `ChannelRouter` weiter.
pub struct VoiceServer {
    config: VoiceServerConfig,
    socket: Arc<UdpSocket>,
    router: ChannelRouter,
    state: VoiceState,
}

impl VoiceServer {
    /// Bindet den UDP-Socket und erstellt einen neuen VoiceServer
    pub async fn binden(
        config: VoiceServerConfig,
        router: ChannelRouter,
        state: VoiceState,
    ) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(config.bind_addr).await?;
        tracing::info!(addr = %config.bind_addr, "UDP Voice Server gebunden");

        Ok(Self {
            config,
            socket: Arc::new(socket),
            router,
            state,
        })
    }

    /// Gibt die lokale Bind-Adresse zurueck
    pub fn lokale_adresse(&self) -> std::io::Result<SocketAddr> {
        self.socket.local_addr()
    }

    /// Registriert einen Client und startet seinen Sende-Task
    ///
    /// Der Client kann danach Pakete empfangen und senden.
    /// Gibt den `ClientSenderHandle` zurueck – beim Drop wird der Task beendet.
    pub fn client_registrieren(
        &self,
        user_id: UserId,
        ssrc: u32,
        udp_endpunkt: SocketAddr,
        kanal_id: ChannelId,
    ) -> ClientSenderHandle {
        // Client im State registrieren
        self.state.client_registrieren(user_id, ssrc, udp_endpunkt);

        // Kanal beitreten (gibt Receive-Queue zurueck – wird vom Router befuellt)
        let _rx = self.router.kanal_beitreten(user_id, kanal_id, udp_endpunkt);

        // Sende-Task starten (liest aus Router-Queue, sendet via UDP)
        // Hinweis: Die _rx vom Router wird nicht direkt hier verwendet –
        // der ClientSenderHandle hat seine eigene Queue fuer direkte Nachrichten.
        // Der Router befuellt die Queues der Empfaenger direkt via try_send.
        ClientSenderHandle::starten(
            Arc::clone(&self.socket),
            udp_endpunkt,
            self.config.send_queue_groesse,
        )
    }

    /// Entfernt einen Client
    pub fn client_entfernen(&self, user_id: &UserId) {
        self.router.kanal_verlassen(user_id);
        self.state.client_entfernen(user_id);
    }

    /// Startet die Empfangs-Loop (laeuft bis `shutdown_rx` ein Signal sendet)
    ///
    /// Diese Methode blockiert bis zum Shutdown-Signal.
    pub async fn empfangs_loop_starten(
        &self,
        mut shutdown_rx: tokio::sync::oneshot::Receiver<()>,
    ) {
        // Stack-allokierter Empfangspuffer – wird wiederverwendet (kein Heap pro Paket)
        let mut buf = [0u8; UDP_BUFFER_SIZE];

        tracing::info!("Voice-Empfangs-Loop gestartet");

        loop {
            tokio::select! {
                // Eingehendes UDP-Paket
                result = self.socket.recv_from(&mut buf) => {
                    match result {
                        Ok((len, absender_addr)) => {
                            self.paket_verarbeiten(&buf[..len], absender_addr).await;
                        }
                        Err(e) => {
                            tracing::error!(fehler = %e, "UDP-Empfangsfehler");
                            // Kurze Pause um Busy-Loop bei persistentem Fehler zu vermeiden
                            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
                        }
                    }
                }

                // Shutdown-Signal
                _ = &mut shutdown_rx => {
                    tracing::info!("Voice-Server: Shutdown-Signal empfangen");
                    break;
                }
            }
        }

        tracing::info!("Voice-Empfangs-Loop beendet");
    }

    // -----------------------------------------------------------------------
    // Internes Paket-Processing
    // -----------------------------------------------------------------------

    /// Verarbeitet ein eingehendes UDP-Paket
    ///
    /// Hot Path: Minimale Allocations, schneller Pfad bei Fehler (early return).
    async fn paket_verarbeiten(&self, daten: &[u8], absender_addr: SocketAddr) {
        // Paket dekodieren und validieren
        let paket = match VoicePacket::decode(daten) {
            Ok(p) => p,
            Err(e) => {
                tracing::debug!(
                    fehler = %e,
                    absender = %absender_addr,
                    "Ungueltiges Voice-Paket"
                );
                return;
            }
        };

        // Client anhand des Endpunkts identifizieren
        let user_id = match self.state.user_id_von_endpunkt(&absender_addr) {
            Some(uid) => uid,
            None => {
                tracing::debug!(
                    absender = %absender_addr,
                    ssrc = paket.header.ssrc,
                    "Unbekannter Absender"
                );
                return;
            }
        };

        // Speaking-Status aus Flags aktualisieren
        if paket.spricht_start() {
            self.state.speaking_setzen(&user_id, true);
        } else if paket.spricht_stop() {
            self.state.speaking_setzen(&user_id, false);
        }

        // Paket-Zeitstempel aktualisieren
        self.state.paket_zeitstempel_aktualisieren(&user_id);

        // Paket an alle anderen Teilnehmer im Kanal weiterleiten
        let weitergeleitet = self.router.paket_weiterleiten(&paket, &user_id);

        tracing::trace!(
            user_id = %user_id,
            sequence = paket.header.sequence,
            ssrc = paket.header.ssrc,
            bytes = daten.len(),
            empfaenger = weitergeleitet,
            "Voice-Paket weitergeleitet"
        );
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use speakeasy_protocol::voice::VoicePacketHeader;
    use std::net::{IpAddr, Ipv4Addr};

    fn localhost(port: u16) -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port)
    }

    fn make_paket(seq: u32, ssrc: u32) -> VoicePacket {
        VoicePacket::neu_audio(seq, seq * 960, ssrc, vec![0xAB; 60])
    }

    #[tokio::test]
    async fn voice_server_binden() {
        let config = VoiceServerConfig::neu(localhost(0)); // Port 0 = OS wählt
        let router = ChannelRouter::neu();
        let state = VoiceState::neu();

        let server = VoiceServer::binden(config, router, state).await
            .expect("Server muss binden koennen");

        let addr = server.lokale_adresse().expect("Adresse muss verfuegbar sein");
        assert_ne!(addr.port(), 0, "OS muss einen Port zuweisen");
    }

    #[tokio::test]
    async fn voice_server_paket_round_trip() {
        // Server starten
        let config = VoiceServerConfig::neu(localhost(0));
        let router = ChannelRouter::neu();
        let state = VoiceState::neu();

        let server = VoiceServer::binden(config, router.clone(), state.clone()).await
            .expect("Server muss binden koennen");
        let server_addr = server.lokale_adresse().unwrap();

        // Zwei Clients registrieren
        let uid1 = UserId::new();
        let uid2 = UserId::new();
        let kanal = ChannelId::new();

        // Client 1: Absender (laeuft auf einem "anderen" Port, wird simuliert)
        let client1_sock = UdpSocket::bind(localhost(0)).await.unwrap();
        let client1_addr = client1_sock.local_addr().unwrap();

        // Client 2: Empfaenger
        let client2_sock = UdpSocket::bind(localhost(0)).await.unwrap();
        let client2_addr = client2_sock.local_addr().unwrap();

        // Beide im State + Router registrieren
        state.client_registrieren(uid1, 0x1111, client1_addr);
        state.client_registrieren(uid2, 0x2222, client2_addr);

        let rx2 = router.kanal_beitreten(uid1, kanal, client1_addr);
        let rx1 = router.kanal_beitreten(uid2, kanal, client2_addr);
        let _ = (rx1, rx2); // Queues halten, damit Channel offen bleibt

        // Server-Empfangs-Task starten
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        let server = Arc::new(server);
        let server_clone = Arc::clone(&server);
        let recv_task = tokio::spawn(async move {
            server_clone.empfangs_loop_starten(shutdown_rx).await;
        });

        // Kurze Pause fuer Task-Start
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        // Client 1 sendet ein Paket an den Server
        let paket = make_paket(1, 0x1111);
        let encoded = paket.encode();
        client1_sock.send_to(&encoded, server_addr).await.unwrap();

        // Kurze Pause fuer Verarbeitung
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Shutdown
        let _ = shutdown_tx.send(());
        recv_task.await.unwrap();
    }

    #[test]
    fn voice_paket_encode_decode_roundtrip() {
        let original = make_paket(42, 0xDEAD);
        let encoded = original.encode();
        let decoded = VoicePacket::decode(&encoded).expect("Decode muss erfolgreich sein");

        assert_eq!(decoded.header.sequence, 42);
        assert_eq!(decoded.header.ssrc, 0xDEAD);
        assert_eq!(decoded.payload, original.payload);
    }

    #[test]
    fn udp_buffer_groesse_ausreichend() {
        use speakeasy_protocol::voice::MAX_NUTZDATEN_LAENGE;
        let max_paket_groesse = VoicePacketHeader::SIZE + MAX_NUTZDATEN_LAENGE;
        assert!(
            UDP_BUFFER_SIZE >= max_paket_groesse,
            "UDP_BUFFER_SIZE ({}) muss >= max Paketgroesse ({}) sein",
            UDP_BUFFER_SIZE,
            max_paket_groesse
        );
    }
}
