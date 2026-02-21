//! Channel Router – Weiterleitung von Voice-Paketen an Kanal-Teilnehmer
//!
//! Der `ChannelRouter` verwaltet alle aktiven Voice-Channels und leitet
//! eingehende Pakete an alle anderen Teilnehmer im selben Kanal weiter (SFU-Stil).
//!
//! ## Design-Entscheidungen
//! - DashMap fuer lock-free concurrent access auf Channel-Liste
//! - Tokio mpsc-Kanaele fuer Send-Queues (kein direktes UDP-Schreiben im Router)
//! - Minimale Allocations: nur eine Vec-Allokation pro Paket fuer Empfaenger-Liste
//!
//! ## Multichannel-Unterstuetzung
//! Ein Client kann genau einem Kanal gleichzeitig angehoeren.
//! Der Router leitet an N-1 Teilnehmer weiter (alle ausser Absender).

use dashmap::DashMap;
use speakeasy_core::types::{ChannelId, UserId};
use speakeasy_protocol::voice::VoicePacket;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::mpsc;

// ---------------------------------------------------------------------------
// Konfiguration
// ---------------------------------------------------------------------------

/// Groesse der Send-Queue pro Client (Pakete)
pub const SEND_QUEUE_GROESSE: usize = 128;

// ---------------------------------------------------------------------------
// Teilnehmer-Info
// ---------------------------------------------------------------------------

/// Informationen ueber einen Kanal-Teilnehmer
#[derive(Debug, Clone)]
pub struct Teilnehmer {
    /// Benutzer-ID
    pub user_id: UserId,
    /// UDP-Zieladresse fuer diesen Teilnehmer
    pub udp_endpunkt: SocketAddr,
    /// Send-Queue: Pakete werden hier hineingelegt und vom UDP-Sender abgeholt
    pub send_tx: mpsc::Sender<Arc<Vec<u8>>>,
}

// ---------------------------------------------------------------------------
// VoiceChannel
// ---------------------------------------------------------------------------

/// Ein aktiver Voice-Kanal mit seinen Teilnehmern
struct VoiceChannel {
    /// Kanal-ID
    kanal_id: ChannelId,
    /// Teilnehmer, indexiert nach UserId
    teilnehmer: DashMap<UserId, Teilnehmer>,
}

impl VoiceChannel {
    fn neu(kanal_id: ChannelId) -> Self {
        Self {
            kanal_id,
            teilnehmer: DashMap::new(),
        }
    }

    /// Fuegt einen Teilnehmer hinzu und gibt seine Empfangs-Queue zurueck
    fn teilnehmer_hinzufuegen(
        &self,
        user_id: UserId,
        udp_endpunkt: SocketAddr,
    ) -> mpsc::Receiver<Arc<Vec<u8>>> {
        let (tx, rx) = mpsc::channel(SEND_QUEUE_GROESSE);
        self.teilnehmer.insert(
            user_id,
            Teilnehmer {
                user_id,
                udp_endpunkt,
                send_tx: tx,
            },
        );
        rx
    }

    /// Entfernt einen Teilnehmer
    fn teilnehmer_entfernen(&self, user_id: &UserId) -> bool {
        self.teilnehmer.remove(user_id).is_some()
    }

    /// Leitet ein Paket an alle Teilnehmer ausser dem Absender weiter
    ///
    /// Erstellt eine Arc<Vec<u8>> einmal und klont nur den Arc (kein Memcpy).
    /// Gibt die Anzahl der erfolgreichen Weiterleitungen zurueck.
    fn paket_weiterleiten(&self, paket_bytes: Arc<Vec<u8>>, absender: &UserId) -> usize {
        let mut weitergeleitet = 0usize;

        self.teilnehmer.iter().for_each(|entry| {
            if &entry.user_id == absender {
                return; // Nicht an Absender zurueckschicken
            }

            // Nicht-blockierend senden – bei voller Queue verwerfen (UDP-Semantik)
            match entry.send_tx.try_send(Arc::clone(&paket_bytes)) {
                Ok(()) => weitergeleitet += 1,
                Err(mpsc::error::TrySendError::Full(_)) => {
                    tracing::warn!(
                        empfaenger = %entry.user_id,
                        kanal = %self.kanal_id,
                        "Send-Queue voll – Paket verworfen"
                    );
                }
                Err(mpsc::error::TrySendError::Closed(_)) => {
                    tracing::debug!(
                        empfaenger = %entry.user_id,
                        "Send-Queue geschlossen (Client getrennt)"
                    );
                }
            }
        });

        weitergeleitet
    }

    /// Anzahl der Teilnehmer in diesem Kanal
    fn teilnehmer_anzahl(&self) -> usize {
        self.teilnehmer.len()
    }
}

// ---------------------------------------------------------------------------
// ChannelRouter
// ---------------------------------------------------------------------------

/// Zentraler Channel Router fuer alle Voice-Kanaele
///
/// Thread-safe und `Clone`-faehig (innerer Arc).
#[derive(Clone)]
pub struct ChannelRouter {
    inner: Arc<ChannelRouterInner>,
}

struct ChannelRouterInner {
    /// Aktive Kanaele, indexiert nach ChannelId
    kanaele: DashMap<ChannelId, VoiceChannel>,
    /// Client -> Kanal Mapping fuer schnelles Leave
    client_kanal: DashMap<UserId, ChannelId>,
}

impl ChannelRouter {
    /// Erstellt einen neuen leeren Channel Router
    pub fn neu() -> Self {
        Self {
            inner: Arc::new(ChannelRouterInner {
                kanaele: DashMap::new(),
                client_kanal: DashMap::new(),
            }),
        }
    }

    /// Ein Client tritt einem Kanal bei
    ///
    /// Falls der Client bereits in einem anderen Kanal ist, wird er zuerst
    /// dort entfernt (automatisches Leave).
    ///
    /// Gibt die Receive-Queue zurueck, aus der der UDP-Sender lesen soll.
    pub fn kanal_beitreten(
        &self,
        user_id: UserId,
        kanal_id: ChannelId,
        udp_endpunkt: SocketAddr,
    ) -> mpsc::Receiver<Arc<Vec<u8>>> {
        // Automatisches Leave aus altem Kanal
        if let Some(alter_kanal) = self.inner.client_kanal.get(&user_id).map(|r| *r) {
            if alter_kanal != kanal_id {
                self.kanal_verlassen_intern(&user_id, &alter_kanal);
            }
        }

        // Kanal erstellen falls nicht vorhanden
        self.inner
            .kanaele
            .entry(kanal_id)
            .or_insert_with(|| VoiceChannel::neu(kanal_id));

        // Teilnehmer hinzufuegen
        let rx = self
            .inner
            .kanaele
            .get(&kanal_id)
            .expect("Kanal wurde gerade erstellt")
            .teilnehmer_hinzufuegen(user_id, udp_endpunkt);

        self.inner.client_kanal.insert(user_id, kanal_id);

        tracing::info!(
            user_id = %user_id,
            kanal_id = %kanal_id,
            endpunkt = %udp_endpunkt,
            "Client beigetreten"
        );

        rx
    }

    /// Ein Client verlasst seinen aktuellen Kanal
    pub fn kanal_verlassen(&self, user_id: &UserId) {
        if let Some((_, kanal_id)) = self.inner.client_kanal.remove(user_id) {
            self.kanal_verlassen_intern(user_id, &kanal_id);
        }
    }

    /// Leitet ein Voice-Paket an alle anderen Teilnehmer im Kanal weiter
    ///
    /// Das Paket wird einmal serialisiert und als `Arc<Vec<u8>>` ohne Kopie
    /// an alle Empfaenger-Queues gesendet.
    ///
    /// Gibt die Anzahl der erfolgreichen Weiterleitungen zurueck (0 bei Fehler).
    pub fn paket_weiterleiten(&self, paket: &VoicePacket, absender: &UserId) -> usize {
        // Kanal des Absenders ermitteln
        let kanal_id = match self.inner.client_kanal.get(absender) {
            Some(k) => *k,
            None => {
                tracing::debug!(user_id = %absender, "Paket von Client ohne Kanal-Zuweisung");
                return 0;
            }
        };

        let kanal = match self.inner.kanaele.get(&kanal_id) {
            Some(k) => k,
            None => {
                tracing::warn!(kanal_id = %kanal_id, "Kanal nicht gefunden");
                return 0;
            }
        };

        // Paket einmal serialisieren, dann als Arc weiterreichen (zero-copy)
        let paket_bytes = Arc::new(paket.encode());
        let count = kanal.paket_weiterleiten(paket_bytes, absender);

        tracing::trace!(
            absender = %absender,
            kanal_id = %kanal_id,
            empfaenger = count,
            "Paket weitergeleitet"
        );

        count
    }

    /// Gibt die Anzahl der Teilnehmer in einem Kanal zurueck
    pub fn teilnehmer_anzahl(&self, kanal_id: &ChannelId) -> usize {
        self.inner
            .kanaele
            .get(kanal_id)
            .map(|k| k.teilnehmer_anzahl())
            .unwrap_or(0)
    }

    /// Gibt alle aktiven Kanal-IDs zurueck
    pub fn aktive_kanaele(&self) -> Vec<ChannelId> {
        self.inner.kanaele.iter().map(|e| *e.key()).collect()
    }

    /// Prueft ob ein Client in einem Kanal ist
    pub fn client_hat_kanal(&self, user_id: &UserId) -> bool {
        self.inner.client_kanal.contains_key(user_id)
    }

    /// Gibt den aktuellen Kanal eines Clients zurueck
    pub fn kanal_von_client(&self, user_id: &UserId) -> Option<ChannelId> {
        self.inner.client_kanal.get(user_id).map(|r| *r)
    }

    // -----------------------------------------------------------------------
    // Interne Hilfsfunktionen
    // -----------------------------------------------------------------------

    fn kanal_verlassen_intern(&self, user_id: &UserId, kanal_id: &ChannelId) {
        let kanal_leer = if let Some(kanal) = self.inner.kanaele.get(kanal_id) {
            kanal.teilnehmer_entfernen(user_id);
            tracing::info!(
                user_id = %user_id,
                kanal_id = %kanal_id,
                "Client verlassen Kanal"
            );
            kanal.teilnehmer_anzahl() == 0
        } else {
            false
        };

        // Leere Kanaele aufraumen
        if kanal_leer {
            self.inner.kanaele.remove(kanal_id);
            tracing::debug!(kanal_id = %kanal_id, "Leerer Kanal entfernt");
        }
    }
}

impl Default for ChannelRouter {
    fn default() -> Self {
        Self::neu()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use speakeasy_protocol::voice::VoicePacket;
    use std::net::{IpAddr, Ipv4Addr};

    fn endpunkt(port: u16) -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port)
    }

    fn test_paket(seq: u32, ssrc: u32) -> VoicePacket {
        VoicePacket::neu_audio(seq, seq * 960, ssrc, vec![0xAB; 60])
    }

    #[tokio::test]
    async fn router_join_und_weiterleitung() {
        let router = ChannelRouter::neu();
        let kanal = ChannelId::new();
        let user1 = UserId::new();
        let user2 = UserId::new();
        let user3 = UserId::new();

        let mut rx1 = router.kanal_beitreten(user1, kanal, endpunkt(20001));
        let mut rx2 = router.kanal_beitreten(user2, kanal, endpunkt(20002));
        let mut rx3 = router.kanal_beitreten(user3, kanal, endpunkt(20003));

        assert_eq!(router.teilnehmer_anzahl(&kanal), 3);

        // user1 sendet ein Paket
        let paket = test_paket(1, 0x1111);
        let weitergeleitet = router.paket_weiterleiten(&paket, &user1);
        assert_eq!(
            weitergeleitet, 2,
            "Paket sollte an user2 und user3 weitergeleitet werden"
        );

        // user1 sollte nichts empfangen (kein Echo)
        assert!(rx1.try_recv().is_err(), "Absender darf kein Echo empfangen");

        // user2 und user3 empfangen das Paket
        let bytes2 = rx2.try_recv().expect("user2 sollte Paket empfangen");
        let bytes3 = rx3.try_recv().expect("user3 sollte Paket empfangen");

        // Paket-Inhalt muss identisch sein (gleicher Arc)
        assert_eq!(bytes2.as_ref(), bytes3.as_ref());

        // Dekodierbares Paket
        let decoded = VoicePacket::decode(&bytes2).expect("Paket muss dekodierbar sein");
        assert_eq!(decoded.header.sequence, 1);
    }

    #[tokio::test]
    async fn router_kein_paket_ohne_kanal() {
        let router = ChannelRouter::neu();
        let user = UserId::new();
        let paket = test_paket(1, 0xAAAA);

        // Paket ohne Kanal-Beitritt
        let count = router.paket_weiterleiten(&paket, &user);
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn router_leave_bereinigt_kanal() {
        let router = ChannelRouter::neu();
        let kanal = ChannelId::new();
        let user1 = UserId::new();
        let user2 = UserId::new();

        let _rx1 = router.kanal_beitreten(user1, kanal, endpunkt(20010));
        let _rx2 = router.kanal_beitreten(user2, kanal, endpunkt(20011));

        assert_eq!(router.teilnehmer_anzahl(&kanal), 2);

        router.kanal_verlassen(&user1);
        assert_eq!(router.teilnehmer_anzahl(&kanal), 1);
        assert!(!router.client_hat_kanal(&user1));
        assert!(router.client_hat_kanal(&user2));

        // Letzter verlasst -> Kanal wird entfernt
        router.kanal_verlassen(&user2);
        assert_eq!(router.aktive_kanaele().len(), 0);
    }

    #[tokio::test]
    async fn router_automatisches_leave_beim_kanal_wechsel() {
        let router = ChannelRouter::neu();
        let kanal_a = ChannelId::new();
        let kanal_b = ChannelId::new();
        let user = UserId::new();

        let _rx_a = router.kanal_beitreten(user, kanal_a, endpunkt(20020));
        assert_eq!(router.kanal_von_client(&user), Some(kanal_a));

        // Wechsel zu Kanal B -> automatisches Leave aus A
        let _rx_b = router.kanal_beitreten(user, kanal_b, endpunkt(20020));
        assert_eq!(router.kanal_von_client(&user), Some(kanal_b));
        assert_eq!(router.teilnehmer_anzahl(&kanal_a), 0);
        assert_eq!(router.teilnehmer_anzahl(&kanal_b), 1);
    }

    #[tokio::test]
    async fn router_multichannel_isolierung() {
        let router = ChannelRouter::neu();
        let kanal_a = ChannelId::new();
        let kanal_b = ChannelId::new();

        let user_a1 = UserId::new();
        let user_a2 = UserId::new();
        let user_b1 = UserId::new();

        let mut rx_a1 = router.kanal_beitreten(user_a1, kanal_a, endpunkt(20030));
        let mut rx_a2 = router.kanal_beitreten(user_a2, kanal_a, endpunkt(20031));
        let mut rx_b1 = router.kanal_beitreten(user_b1, kanal_b, endpunkt(20032));

        // user_a1 sendet in Kanal A
        let paket = test_paket(1, 0xBBBB);
        router.paket_weiterleiten(&paket, &user_a1);

        // user_a2 empfaengt
        assert!(rx_a2.try_recv().is_ok(), "user_a2 sollte empfangen");
        // user_b1 in Kanal B empfaengt NICHT
        assert!(
            rx_b1.try_recv().is_err(),
            "user_b1 in anderem Kanal darf nicht empfangen"
        );
        // user_a1 kein Echo
        assert!(rx_a1.try_recv().is_err(), "user_a1 kein Echo");
    }

    #[test]
    fn router_clone_teilt_state() {
        let router1 = ChannelRouter::neu();
        let router2 = router1.clone();
        let kanal = ChannelId::new();
        let user = UserId::new();

        let _rx = router1.kanal_beitreten(user, kanal, endpunkt(20040));

        // router2 sieht denselben Zustand
        assert_eq!(router2.teilnehmer_anzahl(&kanal), 1);
        assert!(router2.client_hat_kanal(&user));
    }
}
