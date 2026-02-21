//! Quality Telemetrie – Metriken pro Client
//!
//! Sammelt Netzwerk-Qualitaetsmetriken und erstellt periodische Zusammenfassungen.
//!
//! ## Gesammelte Metriken
//! - RTT (aus Ping/Pong-Messungen)
//! - Paketverlust-Rate
//! - Jitter (Standardabweichung der Interarrival-Zeit)
//! - Bitrate (Senden und Empfangen)
//! - Jitter-Buffer-Fuellstand
//!
//! ## Export
//! Alle 5 Sekunden wird ein `TelemetrieSnapshot` erstellt, der ueber ein
//! tokio-Kanal-Interface fuer Observability-Systeme verfuegbar gemacht wird.

use dashmap::DashMap;
use speakeasy_core::types::UserId;
use std::sync::Arc;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Snapshot
// ---------------------------------------------------------------------------

/// Periodischer Telemetrie-Snapshot pro Client
#[derive(Debug, Clone)]
pub struct TelemetrieSnapshot {
    /// Benutzer-ID
    pub user_id: UserId,
    /// Erfassungszeitraum
    pub zeitraum: Duration,
    /// Durchschnittliche RTT in ms
    pub rtt_ms_avg: f64,
    /// Minimale RTT in ms
    pub rtt_ms_min: u32,
    /// Maximale RTT in ms
    pub rtt_ms_max: u32,
    /// Paketverlust-Rate (0.0–1.0)
    pub verlust_rate: f64,
    /// Gesendete Pakete im Zeitraum
    pub pakete_gesendet: u64,
    /// Verlorene Pakete im Zeitraum
    pub pakete_verloren: u64,
    /// Gemessener Jitter in Ticks (Standardabweichung)
    pub jitter_ticks: u32,
    /// Empfangs-Bitrate in bps
    pub empfang_bps: u64,
    /// Sende-Bitrate in bps (Schaetzung basierend auf weitergeleiteten Paketen)
    pub sende_bps: u64,
    /// Jitter-Buffer-Fuellstand (Pakete)
    pub buffer_fuellstand: usize,
}

impl TelemetrieSnapshot {
    /// Gibt eine lesbare Zusammenfassung zurueck
    pub fn zusammenfassung(&self) -> String {
        format!(
            "User {}: RTT={:.0}ms (min={}, max={}) Loss={:.1}% Jitter={}t Empfang={}kbps Sende={}kbps Buffer={}",
            self.user_id,
            self.rtt_ms_avg,
            self.rtt_ms_min,
            self.rtt_ms_max,
            self.verlust_rate * 100.0,
            self.jitter_ticks,
            self.empfang_bps / 1000,
            self.sende_bps / 1000,
            self.buffer_fuellstand,
        )
    }
}

// ---------------------------------------------------------------------------
// Client-Metriken (intern, mutable)
// ---------------------------------------------------------------------------

/// Akkumulierte Metriken eines Clients fuer einen Telemetrie-Zeitraum
struct ClientMetriken {
    user_id: UserId,
    /// RTT-Messungen im Zeitraum
    rtt_messungen: Vec<u32>,
    /// Gesendete Pakete
    pakete_gesendet: u64,
    /// Verlorene Pakete
    pakete_verloren: u64,
    /// Empfangene Bytes
    empfangene_bytes: u64,
    /// Gesendete Bytes (weitergeleitet)
    gesendete_bytes: u64,
    /// Letzter Jitter-Wert (vom JitterBuffer)
    jitter_ticks: u32,
    /// Letzter Buffer-Fuellstand
    buffer_fuellstand: usize,
    /// Startzeitpunkt des aktuellen Zeitraums
    zeitraum_start: Instant,
}

impl ClientMetriken {
    fn neu(user_id: UserId) -> Self {
        Self {
            user_id,
            rtt_messungen: Vec::with_capacity(32),
            pakete_gesendet: 0,
            pakete_verloren: 0,
            empfangene_bytes: 0,
            gesendete_bytes: 0,
            jitter_ticks: 0,
            buffer_fuellstand: 0,
            zeitraum_start: Instant::now(),
        }
    }

    /// Erstellt einen Snapshot und resettet die Akkumulatoren
    fn snapshot_erstellen(&mut self) -> TelemetrieSnapshot {
        let zeitraum = self.zeitraum_start.elapsed();
        let zeitraum_secs = zeitraum.as_secs_f64().max(0.001);

        // RTT-Statistiken
        let (rtt_avg, rtt_min, rtt_max) = if self.rtt_messungen.is_empty() {
            (0.0, 0, 0)
        } else {
            let sum: u64 = self.rtt_messungen.iter().map(|&r| r as u64).sum();
            let avg = sum as f64 / self.rtt_messungen.len() as f64;
            let min = *self.rtt_messungen.iter().min().unwrap();
            let max = *self.rtt_messungen.iter().max().unwrap();
            (avg, min, max)
        };

        // Verlust-Rate
        let verlust_rate = if self.pakete_gesendet > 0 {
            self.pakete_verloren as f64 / self.pakete_gesendet as f64
        } else {
            0.0
        };

        // Bitraten
        let empfang_bps = ((self.empfangene_bytes * 8) as f64 / zeitraum_secs) as u64;
        let sende_bps = ((self.gesendete_bytes * 8) as f64 / zeitraum_secs) as u64;

        let snapshot = TelemetrieSnapshot {
            user_id: self.user_id,
            zeitraum,
            rtt_ms_avg: rtt_avg,
            rtt_ms_min: rtt_min,
            rtt_ms_max: rtt_max,
            verlust_rate,
            pakete_gesendet: self.pakete_gesendet,
            pakete_verloren: self.pakete_verloren,
            jitter_ticks: self.jitter_ticks,
            empfang_bps,
            sende_bps,
            buffer_fuellstand: self.buffer_fuellstand,
        };

        // Akkumulatoren resetten
        self.rtt_messungen.clear();
        self.pakete_gesendet = 0;
        self.pakete_verloren = 0;
        self.empfangene_bytes = 0;
        self.gesendete_bytes = 0;
        self.zeitraum_start = Instant::now();

        snapshot
    }
}

// ---------------------------------------------------------------------------
// VoiceTelemetry
// ---------------------------------------------------------------------------

/// Intervall fuer periodische Telemetrie-Snapshots
pub const TELEMETRIE_INTERVALL: Duration = Duration::from_secs(5);

/// Zentrales Telemetrie-System fuer alle Voice-Clients
///
/// Thread-safe durch DashMap + Arc.
/// Der Export-Task laeuft separat via `starten()`.
#[derive(Clone)]
pub struct VoiceTelemetry {
    inner: Arc<TelemetrieInner>,
}

struct TelemetrieInner {
    /// Metriken pro Client (unter parking_lot Mutex fuer Akkumulation)
    clients: DashMap<UserId, parking_lot::Mutex<ClientMetriken>>,
    /// Kanal fuer Snapshot-Export
    export_tx: tokio::sync::broadcast::Sender<TelemetrieSnapshot>,
}

impl VoiceTelemetry {
    /// Erstellt ein neues Telemetrie-System
    ///
    /// Gibt auch den Broadcast-Receiver zurueck, ueber den Snapshots empfangen werden.
    pub fn neu() -> (Self, tokio::sync::broadcast::Receiver<TelemetrieSnapshot>) {
        let (tx, rx) = tokio::sync::broadcast::channel(256);
        let telemetry = Self {
            inner: Arc::new(TelemetrieInner {
                clients: DashMap::new(),
                export_tx: tx,
            }),
        };
        (telemetry, rx)
    }

    /// Registriert einen neuen Client fuer Telemetrie-Erfassung
    pub fn client_registrieren(&self, user_id: UserId) {
        self.inner.clients.insert(
            user_id,
            parking_lot::Mutex::new(ClientMetriken::neu(user_id)),
        );
    }

    /// Entfernt einen Client aus der Telemetrie
    pub fn client_entfernen(&self, user_id: &UserId) {
        self.inner.clients.remove(user_id);
    }

    /// Aktualisiert die RTT eines Clients (aus Ping/Pong)
    pub fn rtt_aktualisieren(&self, user_id: &UserId, rtt_ms: u32) {
        if let Some(entry) = self.inner.clients.get(user_id) {
            entry.lock().rtt_messungen.push(rtt_ms);
        }
    }

    /// Meldet ein gesendetes (weitergeleitetes) Paket mit Byteanzahl
    pub fn paket_gesendet(&self, user_id: &UserId, bytes: usize) {
        if let Some(entry) = self.inner.clients.get(user_id) {
            let mut m = entry.lock();
            m.pakete_gesendet += 1;
            m.gesendete_bytes += bytes as u64;
        }
    }

    /// Meldet ein empfangenes Paket mit Byteanzahl
    pub fn paket_empfangen(&self, user_id: &UserId, bytes: usize) {
        if let Some(entry) = self.inner.clients.get(user_id) {
            let mut m = entry.lock();
            m.empfangene_bytes += bytes as u64;
        }
    }

    /// Meldet ein verlorenes Paket
    pub fn paket_verloren(&self, user_id: &UserId) {
        if let Some(entry) = self.inner.clients.get(user_id) {
            entry.lock().pakete_verloren += 1;
        }
    }

    /// Aktualisiert Jitter und Buffer-Fuellstand (vom JitterBuffer)
    pub fn jitter_aktualisieren(
        &self,
        user_id: &UserId,
        jitter_ticks: u32,
        buffer_fuellstand: usize,
    ) {
        if let Some(entry) = self.inner.clients.get(user_id) {
            let mut m = entry.lock();
            m.jitter_ticks = jitter_ticks;
            m.buffer_fuellstand = buffer_fuellstand;
        }
    }

    /// Erstellt sofort Snapshots fuer alle Clients und sendet sie
    ///
    /// Wird normalerweise vom periodischen Task aufgerufen.
    pub fn snapshots_erstellen(&self) -> Vec<TelemetrieSnapshot> {
        let mut snapshots = Vec::with_capacity(self.inner.clients.len());

        self.inner.clients.iter().for_each(|entry| {
            let snapshot = entry.value().lock().snapshot_erstellen();
            tracing::debug!("{}", snapshot.zusammenfassung());

            // An Subscriber senden (Fehler ignorieren wenn keine Subscriber)
            let _ = self.inner.export_tx.send(snapshot.clone());
            snapshots.push(snapshot);
        });

        snapshots
    }

    /// Startet den periodischen Telemetrie-Task
    ///
    /// Gibt ein `JoinHandle` zurueck. Der Task laeuft bis der `VoiceTelemetry`
    /// gedroppt wird (Arc-Zaehler auf 0).
    pub fn starten(&self, intervall: Duration) -> tokio::task::JoinHandle<()> {
        let telemetry = self.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(intervall);
            ticker.tick().await; // Ersten Tick ueberspringen

            loop {
                ticker.tick().await;
                let snapshots = telemetry.snapshots_erstellen();
                if !snapshots.is_empty() {
                    tracing::info!(clients = snapshots.len(), "Telemetrie-Snapshot erstellt");
                }
            }
        })
    }

    /// Gibt die Anzahl der ueberwachten Clients zurueck
    pub fn client_anzahl(&self) -> usize {
        self.inner.clients.len()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn telemetrie_client_registrieren() {
        let (tele, _rx) = VoiceTelemetry::neu();
        let uid = UserId::new();

        tele.client_registrieren(uid);
        assert_eq!(tele.client_anzahl(), 1);

        tele.client_entfernen(&uid);
        assert_eq!(tele.client_anzahl(), 0);
    }

    #[test]
    fn telemetrie_rtt_akkumulation() {
        let (tele, _rx) = VoiceTelemetry::neu();
        let uid = UserId::new();

        tele.client_registrieren(uid);
        tele.rtt_aktualisieren(&uid, 20);
        tele.rtt_aktualisieren(&uid, 30);
        tele.rtt_aktualisieren(&uid, 40);

        let snapshots = tele.snapshots_erstellen();
        assert_eq!(snapshots.len(), 1);

        let snap = &snapshots[0];
        assert!(
            (snap.rtt_ms_avg - 30.0).abs() < 0.01,
            "Durchschnitt sollte 30ms sein"
        );
        assert_eq!(snap.rtt_ms_min, 20);
        assert_eq!(snap.rtt_ms_max, 40);
    }

    #[test]
    fn telemetrie_verlust_rate() {
        let (tele, _rx) = VoiceTelemetry::neu();
        let uid = UserId::new();

        tele.client_registrieren(uid);

        // 10 gesendet, 2 verloren
        for _ in 0..10 {
            tele.paket_gesendet(&uid, 100);
        }
        for _ in 0..2 {
            tele.paket_verloren(&uid);
        }

        let snapshots = tele.snapshots_erstellen();
        let snap = &snapshots[0];

        assert!(
            (snap.verlust_rate - 0.2).abs() < 0.01,
            "Verlust-Rate sollte 20% sein"
        );
        assert_eq!(snap.pakete_gesendet, 10);
        assert_eq!(snap.pakete_verloren, 2);
    }

    #[test]
    fn telemetrie_bitrate_berechnung() {
        let (tele, _rx) = VoiceTelemetry::neu();
        let uid = UserId::new();

        tele.client_registrieren(uid);

        // 1000 Bytes empfangen
        tele.paket_empfangen(&uid, 1000);

        let snapshots = tele.snapshots_erstellen();
        let snap = &snapshots[0];

        // Bitrate > 0 (Zeitraum sehr kurz, aber Bytes vorhanden)
        assert!(snap.empfang_bps > 0, "Empfangs-Bitrate muss > 0 sein");
    }

    #[test]
    fn telemetrie_reset_nach_snapshot() {
        let (tele, _rx) = VoiceTelemetry::neu();
        let uid = UserId::new();

        tele.client_registrieren(uid);
        tele.rtt_aktualisieren(&uid, 50);
        tele.paket_gesendet(&uid, 200);

        // Erster Snapshot
        let snap1 = tele.snapshots_erstellen();
        assert!(!snap1.is_empty());

        // Zweiter Snapshot (kein neue Daten)
        let snap2 = tele.snapshots_erstellen();
        assert_eq!(
            snap2[0].pakete_gesendet, 0,
            "Akkumulatoren muessen nach Snapshot resettet sein"
        );
    }

    #[tokio::test]
    async fn telemetrie_broadcast_export() {
        let (tele, mut rx) = VoiceTelemetry::neu();
        let uid = UserId::new();

        tele.client_registrieren(uid);
        tele.rtt_aktualisieren(&uid, 25);

        // Snapshot erstellen -> wird via Broadcast gesendet
        tele.snapshots_erstellen();

        // Empfanger sollte Snapshot erhalten
        let snap = rx
            .try_recv()
            .expect("Snapshot sollte via Broadcast ankommen");
        assert_eq!(snap.user_id, uid);
    }
}
