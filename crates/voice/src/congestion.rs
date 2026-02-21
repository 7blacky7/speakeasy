//! Congestion Controller – Netzwerkqualitaet pro Client
//!
//! Ueberwacht RTT, Packet Loss und Jitter und empfiehlt Bitrate-Anpassungen.
//!
//! ## Strategie
//! - **Loss-basiert**: Bei > 5% Paketverlust -> Bitrate reduzieren
//! - **Delay-basiert**: Bei steigendem RTT (Trend > Schwellwert) -> warnen
//! - **Recovery**: Bei stabiler Verbindung langsam wieder erhoehen
//!
//! ## Performance
//! - Alle Berechnungen O(1), keine Allocations im Hot Path
//! - Atomare Zustandsspeicherung wo moeglich

use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Konfiguration
// ---------------------------------------------------------------------------

/// Konfiguration fuer den Congestion Controller
#[derive(Debug, Clone)]
pub struct CongestionConfig {
    /// Schwellwert fuer Paketverlust-Alarm (0.0–1.0)
    pub verlust_schwellwert: f64,
    /// RTT-Anstieg in ms ab dem gewarnt wird
    pub rtt_warn_delta_ms: u32,
    /// Minimale Bitrate (kbps)
    pub min_bitrate_kbps: u16,
    /// Maximale Bitrate (kbps)
    pub max_bitrate_kbps: u16,
    /// Faktor fuer Bitrate-Reduzierung bei Loss (0.0–1.0)
    pub reduction_factor: f64,
    /// Faktor fuer Bitrate-Erhoehung bei Erholung (pro Intervall)
    pub recovery_factor: f64,
    /// Messintervall fuer Statistik-Reset
    pub messintervall: Duration,
    /// Anzahl stabiler Intervalle vor Recovery
    pub stabile_intervalle_fuer_recovery: u32,
}

impl Default for CongestionConfig {
    fn default() -> Self {
        Self {
            verlust_schwellwert: 0.05, // 5% Verlust
            rtt_warn_delta_ms: 50,     // 50ms RTT-Anstieg
            min_bitrate_kbps: 8,
            max_bitrate_kbps: 510,
            reduction_factor: 0.75, // 25% Reduzierung
            recovery_factor: 1.05,  // 5% Erhoehung pro Intervall
            messintervall: Duration::from_secs(1),
            stabile_intervalle_fuer_recovery: 3,
        }
    }
}

// ---------------------------------------------------------------------------
// Aktionen und Empfehlungen
// ---------------------------------------------------------------------------

/// Empfehlung des Congestion Controllers an den Client
#[derive(Debug, Clone, PartialEq)]
pub enum CongestionAktion {
    /// Verbindung stabil – keine Aenderung noetig
    Stabil,
    /// Bitrate auf den angegebenen Wert reduzieren
    BitrateReduzieren { neue_bitrate_kbps: u16 },
    /// Bitrate erhoehen (Recovery nach stabiler Phase)
    BitrateErhoehen { neue_bitrate_kbps: u16 },
    /// Warnung: RTT steigt, aber noch kein Eingriff
    RttWarnung { rtt_ms: u32, delta_ms: i64 },
    /// Kritisch: Hoher Verlust UND hoher RTT
    Kritisch { verlust_prozent: f64, rtt_ms: u32 },
}

// ---------------------------------------------------------------------------
// Metriken-Snapshot
// ---------------------------------------------------------------------------

/// Aktuelle Netzwerk-Metriken eines Clients
#[derive(Debug, Clone, Default)]
pub struct NetzwerkMetriken {
    /// Round-Trip-Time in Millisekunden
    pub rtt_ms: u32,
    /// Paketverlust-Rate (0.0 = kein Verlust, 1.0 = 100% Verlust)
    pub verlust_rate: f64,
    /// Jitter in Ticks (Standardabweichung der Interarrival-Zeit)
    pub jitter_ticks: u32,
    /// Gesendete Pakete seit letztem Reset
    pub gesendete_pakete: u64,
    /// Verlorene Pakete seit letztem Reset
    pub verlorene_pakete: u64,
    /// Empfangene Bytes pro Sekunde
    pub empfang_bitrate_bps: u32,
}

// ---------------------------------------------------------------------------
// CongestionController
// ---------------------------------------------------------------------------

/// Congestion Controller – ueberwacht Netzwerkqualitaet und empfiehlt Bitrate-Anpassungen
pub struct CongestionController {
    config: CongestionConfig,
    /// Aktuelle Bitrate-Empfehlung
    aktuelle_bitrate_kbps: u16,
    /// Letzte gemessene RTT
    letzte_rtt_ms: u32,
    /// Letzte RTT fuer Delta-Berechnung
    vorherige_rtt_ms: u32,
    /// Gesendete Pakete im aktuellen Intervall
    gesendete_pakete: u64,
    /// Verlorene Pakete im aktuellen Intervall
    verlorene_pakete: u64,
    /// Empfangene Bytes im aktuellen Intervall
    empfangene_bytes: u64,
    /// Beginn des aktuellen Messintervalls
    intervall_start: Instant,
    /// Anzahl stabiler Intervalle in Folge
    stabile_intervalle: u32,
    /// Letzte berechnete Verlust-Rate
    letzte_verlust_rate: f64,
}

impl CongestionController {
    /// Erstellt einen neuen Controller mit Standardkonfiguration und Anfangs-Bitrate
    pub fn neu(start_bitrate_kbps: u16) -> Self {
        Self::mit_config(CongestionConfig::default(), start_bitrate_kbps)
    }

    /// Erstellt einen Controller mit eigener Konfiguration
    pub fn mit_config(config: CongestionConfig, start_bitrate_kbps: u16) -> Self {
        let bitrate = start_bitrate_kbps
            .max(config.min_bitrate_kbps)
            .min(config.max_bitrate_kbps);
        Self {
            aktuelle_bitrate_kbps: bitrate,
            config,
            letzte_rtt_ms: 0,
            vorherige_rtt_ms: 0,
            gesendete_pakete: 0,
            verlorene_pakete: 0,
            empfangene_bytes: 0,
            intervall_start: Instant::now(),
            stabile_intervalle: 0,
            letzte_verlust_rate: 0.0,
        }
    }

    /// Aktualisiert die RTT-Messung (aus Ping/Pong)
    pub fn rtt_aktualisieren(&mut self, rtt_ms: u32) {
        self.vorherige_rtt_ms = self.letzte_rtt_ms;
        self.letzte_rtt_ms = rtt_ms;
    }

    /// Meldet ein gesendetes Paket
    pub fn paket_gesendet(&mut self) {
        self.gesendete_pakete += 1;
    }

    /// Meldet ein verlorenes Paket
    pub fn paket_verloren(&mut self) {
        self.verlorene_pakete += 1;
    }

    /// Meldet empfangene Bytes (fuer Bitrate-Berechnung)
    pub fn bytes_empfangen(&mut self, bytes: u64) {
        self.empfangene_bytes += bytes;
    }

    /// Gibt die aktuelle Bitrate-Empfehlung zurueck
    pub fn aktuelle_bitrate_kbps(&self) -> u16 {
        self.aktuelle_bitrate_kbps
    }

    /// Gibt die aktuellen Metriken zurueck
    pub fn metriken(&self) -> NetzwerkMetriken {
        NetzwerkMetriken {
            rtt_ms: self.letzte_rtt_ms,
            verlust_rate: self.letzte_verlust_rate,
            jitter_ticks: 0, // wird vom JitterBuffer beigesteuert
            gesendete_pakete: self.gesendete_pakete,
            verlorene_pakete: self.verlorene_pakete,
            empfang_bitrate_bps: 0, // wird im Tick berechnet
        }
    }

    /// Fuehrt eine Auswertung durch und gibt eine Empfehlung zurueck
    ///
    /// Sollte periodisch aufgerufen werden (z.B. alle 1s via tokio::time::interval).
    /// Resettet interne Zaehler nach jedem Aufruf.
    pub fn auswerten(&mut self) -> CongestionAktion {
        let elapsed = self.intervall_start.elapsed();

        // Verlust-Rate berechnen
        let verlust_rate = if self.gesendete_pakete > 0 {
            self.verlorene_pakete as f64 / self.gesendete_pakete as f64
        } else {
            0.0
        };
        self.letzte_verlust_rate = verlust_rate;

        // RTT-Delta berechnen
        let rtt_delta = self.letzte_rtt_ms as i64 - self.vorherige_rtt_ms as i64;

        // Metriken loggen
        tracing::debug!(
            rtt_ms = self.letzte_rtt_ms,
            verlust_rate = format!("{:.1}%", verlust_rate * 100.0),
            gesendete_pakete = self.gesendete_pakete,
            verlorene_pakete = self.verlorene_pakete,
            "Congestion-Auswertung"
        );

        // Intervall-Zaehler resetten
        self.gesendete_pakete = 0;
        self.verlorene_pakete = 0;
        self.empfangene_bytes = 0;
        self.intervall_start = Instant::now();
        let _ = elapsed;

        // Entscheidungslogik
        let hoher_verlust = verlust_rate > self.config.verlust_schwellwert;
        let hoher_rtt = self.letzte_rtt_ms > 200;
        let rtt_steigt = rtt_delta > self.config.rtt_warn_delta_ms as i64;

        if hoher_verlust && hoher_rtt {
            // Kritisch: Bitrate stark reduzieren
            self.stabile_intervalle = 0;
            let neue_bitrate = ((self.aktuelle_bitrate_kbps as f64
                * self.config.reduction_factor
                * self.config.reduction_factor)
                .round() as u16)
                .max(self.config.min_bitrate_kbps);
            self.aktuelle_bitrate_kbps = neue_bitrate;
            tracing::warn!(
                neue_bitrate_kbps = neue_bitrate,
                verlust_prozent = verlust_rate * 100.0,
                rtt_ms = self.letzte_rtt_ms,
                "Kritische Netzwerklage – Bitrate stark reduziert"
            );
            return CongestionAktion::Kritisch {
                verlust_prozent: verlust_rate * 100.0,
                rtt_ms: self.letzte_rtt_ms,
            };
        }

        if hoher_verlust {
            // Loss-basierte Reduzierung
            self.stabile_intervalle = 0;
            let neue_bitrate = ((self.aktuelle_bitrate_kbps as f64 * self.config.reduction_factor)
                .round() as u16)
                .max(self.config.min_bitrate_kbps);
            self.aktuelle_bitrate_kbps = neue_bitrate;
            tracing::info!(
                neue_bitrate_kbps = neue_bitrate,
                verlust_prozent = verlust_rate * 100.0,
                "Paketverlust > Schwellwert – Bitrate reduziert"
            );
            return CongestionAktion::BitrateReduzieren {
                neue_bitrate_kbps: neue_bitrate,
            };
        }

        if rtt_steigt {
            // Delay-basierte Warnung (kein Eingriff, aber Client informieren)
            self.stabile_intervalle = 0;
            tracing::debug!(
                rtt_ms = self.letzte_rtt_ms,
                delta_ms = rtt_delta,
                "RTT steigt – Warnung"
            );
            return CongestionAktion::RttWarnung {
                rtt_ms: self.letzte_rtt_ms,
                delta_ms: rtt_delta,
            };
        }

        // Stabil – Recovery pruefen
        self.stabile_intervalle += 1;
        if self.stabile_intervalle >= self.config.stabile_intervalle_fuer_recovery {
            let neue_bitrate = ((self.aktuelle_bitrate_kbps as f64 * self.config.recovery_factor)
                .round() as u16)
                .min(self.config.max_bitrate_kbps);
            if neue_bitrate > self.aktuelle_bitrate_kbps {
                self.aktuelle_bitrate_kbps = neue_bitrate;
                tracing::debug!(
                    neue_bitrate_kbps = neue_bitrate,
                    "Stabile Verbindung – Bitrate erhoehen"
                );
                return CongestionAktion::BitrateErhoehen {
                    neue_bitrate_kbps: neue_bitrate,
                };
            }
        }

        CongestionAktion::Stabil
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn congestion_stabil_bei_kein_verlust() {
        let mut ctrl = CongestionController::neu(64);
        ctrl.rtt_aktualisieren(20);

        // 100 Pakete, kein Verlust
        for _ in 0..100 {
            ctrl.paket_gesendet();
        }

        let aktion = ctrl.auswerten();
        // Kein Verlust, RTT stabil -> Stabil oder Recovery
        assert!(
            matches!(
                aktion,
                CongestionAktion::Stabil | CongestionAktion::BitrateErhoehen { .. }
            ),
            "Unerwartete Aktion bei stabilem Netz: {:?}",
            aktion
        );
    }

    #[test]
    fn congestion_bitrate_reduzieren_bei_hohem_verlust() {
        let mut ctrl = CongestionController::neu(64);
        ctrl.rtt_aktualisieren(30);

        // 100 Pakete, 10 verloren (10% > 5% Schwellwert)
        for _ in 0..100 {
            ctrl.paket_gesendet();
        }
        for _ in 0..10 {
            ctrl.paket_verloren();
        }

        let aktion = ctrl.auswerten();
        assert!(
            matches!(aktion, CongestionAktion::BitrateReduzieren { .. }),
            "Erwartet BitrateReduzieren bei 10% Verlust, bekam: {:?}",
            aktion
        );
    }

    #[test]
    fn congestion_bitrate_reduzierung_berechnung() {
        let config = CongestionConfig {
            reduction_factor: 0.75,
            min_bitrate_kbps: 8,
            max_bitrate_kbps: 510,
            ..Default::default()
        };
        let mut ctrl = CongestionController::mit_config(config, 64);
        ctrl.rtt_aktualisieren(20);

        for _ in 0..100 {
            ctrl.paket_gesendet();
        }
        for _ in 0..10 {
            ctrl.paket_verloren();
        }

        let aktion = ctrl.auswerten();
        if let CongestionAktion::BitrateReduzieren { neue_bitrate_kbps } = aktion {
            // 64 * 0.75 = 48
            assert_eq!(neue_bitrate_kbps, 48, "Falsche Bitrate nach Reduzierung");
        } else {
            panic!("Falsche Aktion: {:?}", aktion);
        }
        assert_eq!(ctrl.aktuelle_bitrate_kbps(), 48);
    }

    #[test]
    fn congestion_nicht_unter_minimum() {
        let config = CongestionConfig {
            reduction_factor: 0.1, // Extrem aggressiv
            min_bitrate_kbps: 8,
            max_bitrate_kbps: 510,
            ..Default::default()
        };
        let mut ctrl = CongestionController::mit_config(config, 8);
        ctrl.rtt_aktualisieren(20);

        for _ in 0..100 {
            ctrl.paket_gesendet();
        }
        for _ in 0..10 {
            ctrl.paket_verloren();
        }

        ctrl.auswerten();
        assert!(
            ctrl.aktuelle_bitrate_kbps() >= 8,
            "Bitrate unter Minimum gesunken"
        );
    }

    #[test]
    fn congestion_recovery_nach_stabilen_intervallen() {
        let config = CongestionConfig {
            stabile_intervalle_fuer_recovery: 2,
            recovery_factor: 1.10, // 10% pro Intervall
            verlust_schwellwert: 0.05,
            ..Default::default()
        };
        let mut ctrl = CongestionController::mit_config(config, 40); // Unter Maximum
        ctrl.rtt_aktualisieren(20);

        // 3 stabile Intervalle (kein Verlust, kein RTT-Anstieg)
        for _ in 0..3 {
            ctrl.auswerten(); // Kein Paket gesendet = kein Verlust
        }

        // Nach 2 stabilen Intervallen sollte Recovery einsetzen
        assert!(
            ctrl.aktuelle_bitrate_kbps() > 40,
            "Keine Recovery nach stabilen Intervallen (aktuell: {} kbps)",
            ctrl.aktuelle_bitrate_kbps()
        );
    }

    #[test]
    fn congestion_rtt_warnung() {
        let mut ctrl = CongestionController::neu(64);
        ctrl.rtt_aktualisieren(30); // Erste RTT
        ctrl.auswerten(); // Ersten Wert einlesen

        ctrl.rtt_aktualisieren(100); // RTT stark gestiegen (+70ms > 50ms Schwellwert)

        for _ in 0..10 {
            ctrl.paket_gesendet();
        }
        // Kein Verlust

        let aktion = ctrl.auswerten();
        assert!(
            matches!(aktion, CongestionAktion::RttWarnung { .. }),
            "Erwartet RTT-Warnung, bekam: {:?}",
            aktion
        );
    }

    #[test]
    fn congestion_kritisch_bei_verlust_und_rtt() {
        let mut ctrl = CongestionController::neu(64);
        ctrl.rtt_aktualisieren(250); // Hoher RTT

        for _ in 0..100 {
            ctrl.paket_gesendet();
        }
        for _ in 0..20 {
            ctrl.paket_verloren();
        } // 20% Verlust

        let aktion = ctrl.auswerten();
        assert!(
            matches!(aktion, CongestionAktion::Kritisch { .. }),
            "Erwartet Kritisch, bekam: {:?}",
            aktion
        );
    }
}
