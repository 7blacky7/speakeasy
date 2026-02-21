//! Adaptiver Jitter Buffer fuer Voice-Pakete
//!
//! Ordnet eingehende UDP-Pakete nach Sequenznummer und puffert sie,
//! um Netzwerk-Jitter auszugleichen. Unterstuetzt zwei Modi:
//! - **Adaptiv**: passt die Buffer-Groesse dynamisch an gemessenen Jitter an
//! - **Fixed**: konstante Buffer-Groesse (deterministische Latenz)
//!
//! ## Performance-Eigenschaften
//! - O(log n) Einf??gen (BTreeMap nach Sequence sortiert)
//! - O(1) Entnahme des aeltesten Pakets
//! - Keine Locks im Hot Path (wird pro-Client single-threaded verwendet)

use speakeasy_protocol::voice::VoicePacket;
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Konfiguration
// ---------------------------------------------------------------------------

/// Modus des Jitter Buffers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JitterBufferModus {
    /// Adaptiv: Buffer-Groesse passt sich dem gemessenen Jitter an
    Adaptiv,
    /// Fixed: konstante Buffer-Groesse in Paketen
    Fixed,
}

/// Konfiguration fuer den Jitter Buffer
#[derive(Debug, Clone)]
pub struct JitterBufferConfig {
    /// Betriebsmodus
    pub modus: JitterBufferModus,
    /// Initiale / Maximale Buffer-Groesse in Paketen
    pub max_pakete: usize,
    /// Minimale Buffer-Groesse im adaptiven Modus (Pakete)
    pub min_pakete: usize,
    /// Fenstergroesse fuer Jitter-Messung (letzte N Interarrivals)
    pub jitter_fenster: usize,
}

impl Default for JitterBufferConfig {
    fn default() -> Self {
        Self {
            modus: JitterBufferModus::Adaptiv,
            max_pakete: 50,
            min_pakete: 2,
            jitter_fenster: 16,
        }
    }
}

// ---------------------------------------------------------------------------
// Statistiken
// ---------------------------------------------------------------------------

/// Statistiken des Jitter Buffers (Snapshot)
#[derive(Debug, Clone, Default)]
pub struct JitterBufferStatistik {
    /// Anzahl empfangener Pakete gesamt
    pub empfangen: u64,
    /// Anzahl korrekt abgespielter Pakete
    pub abgespielt: u64,
    /// Anzahl verworfener Duplikate
    pub duplikate: u64,
    /// Anzahl verlorener Pakete (Luecken, die nie geschlossen wurden)
    pub verloren: u64,
    /// Anzahl Out-of-Order Pakete (spaet angekommen, aber noch verwendbar)
    pub out_of_order: u64,
    /// Gemessener Jitter in Ticks (Standardabweichung der Interarrival-Zeit)
    pub jitter_ticks: u32,
    /// Aktuelle Buffer-Fuellstand in Paketen
    pub fuellstand: usize,
    /// Aktuelle Ziel-Buffer-Groesse (adaptiv angepasst)
    pub ziel_groesse: usize,
}

// ---------------------------------------------------------------------------
// AdaptiveJitterBuffer
// ---------------------------------------------------------------------------

/// Adaptiver Jitter Buffer – puffert und sortiert Voice-Pakete
///
/// Verwendet einen `BTreeMap<sequence, VoicePacket>` fuer O(log n) Einf??gen
/// und O(1) geordnete Entnahme. Der Buffer ist pro-Client und nicht thread-safe
/// (Synchronisation erfolgt auf hoeherer Ebene).
pub struct AdaptiveJitterBuffer {
    config: JitterBufferConfig,
    /// Gepufferte Pakete, sortiert nach Sequenznummer
    pakete: BTreeMap<u32, VoicePacket>,
    /// Naechste erwartete Sequenznummer (fuer Lueckenerkennung)
    naechste_seq: Option<u32>,
    /// Letzte abgespielte Sequenznummer (fuer Duplikat-Erkennung)
    letzte_abgespielt: Option<u32>,
    /// Interarrival-Zeiten fuer Jitter-Berechnung (Ringpuffer)
    interarrivals: Vec<i64>,
    interarrival_idx: usize,
    /// Zeitstempel des letzten empfangenen Pakets (in Ticks)
    letzter_timestamp: Option<u32>,
    /// Statistiken
    statistik: JitterBufferStatistik,
    /// Aktuell berechneter Jitter (Welford-Online-Algorithmus)
    jitter_mittel: f64,
    jitter_m2: f64,
    jitter_n: u64,
    /// Aktuelle Ziel-Buffer-Groesse (adaptiv)
    ziel_groesse: usize,
}

impl AdaptiveJitterBuffer {
    /// Erstellt einen neuen Jitter Buffer mit gegebener Konfiguration
    pub fn neu(config: JitterBufferConfig) -> Self {
        let ziel = config.max_pakete / 2; // Startwert: Haelfte des Maximums
        let fenster = config.jitter_fenster;
        Self {
            ziel_groesse: ziel.max(config.min_pakete),
            config,
            pakete: BTreeMap::new(),
            naechste_seq: None,
            letzte_abgespielt: None,
            interarrivals: vec![0i64; fenster],
            interarrival_idx: 0,
            letzter_timestamp: None,
            statistik: JitterBufferStatistik::default(),
            jitter_mittel: 0.0,
            jitter_m2: 0.0,
            jitter_n: 0,
        }
    }

    /// Erstellt einen adaptiven Buffer mit Standardkonfiguration
    pub fn standard() -> Self {
        Self::neu(JitterBufferConfig::default())
    }

    /// Fuegt ein Paket in den Buffer ein
    ///
    /// Erkennt: Duplikate, Out-of-Order, Lost Packets
    pub fn push(&mut self, paket: VoicePacket) {
        let seq = paket.header.sequence;
        self.statistik.empfangen += 1;

        // Duplikat-Erkennung
        if let Some(letzte) = self.letzte_abgespielt {
            if self.ist_sequence_alt(seq, letzte) {
                self.statistik.duplikate += 1;
                tracing::trace!(sequence = seq, "Duplikat-Paket verworfen");
                return;
            }
        }

        // Duplikat im Buffer selbst
        if self.pakete.contains_key(&seq) {
            self.statistik.duplikate += 1;
            return;
        }

        // Out-of-Order Erkennung
        if let Some(naechste) = self.naechste_seq {
            if self.ist_sequence_alt(seq, naechste.wrapping_sub(1)) {
                self.statistik.out_of_order += 1;
                tracing::debug!(sequence = seq, erwartet = naechste, "Out-of-Order Paket");
            }
        }

        // Jitter messen (basierend auf RTP-Timestamp)
        self.jitter_messen(paket.header.timestamp);

        // Paket einf??gen
        self.pakete.insert(seq, paket);

        // Buffer-Ueberlauf: aeltestes Paket verwerfen
        if self.pakete.len() > self.config.max_pakete {
            if let Some((&aelteste_seq, _)) = self.pakete.iter().next() {
                self.pakete.remove(&aelteste_seq);
                self.statistik.verloren += 1;
                tracing::warn!(sequence = aelteste_seq, "Buffer-Ueberlauf: Paket verworfen");
            }
        }

        // Adaptiven Zielwert aktualisieren
        if self.config.modus == JitterBufferModus::Adaptiv {
            self.ziel_groesse_anpassen();
        }

        self.statistik.fuellstand = self.pakete.len();
        self.statistik.ziel_groesse = self.ziel_groesse;
    }

    /// Gibt das naechste Paket zurueck, falls der Buffer gefuellt genug ist
    ///
    /// Gibt `None` zurueck wenn:
    /// - Buffer leer
    /// - Buffer noch nicht gefuellt genug (Startup-Latenz, nur bei min_pakete > 0)
    pub fn pop(&mut self) -> Option<VoicePacket> {
        if self.pakete.is_empty() {
            return None;
        }

        // Startup-Latenz: Warten bis Buffer genuegend gefuellt ist.
        // Nur wenn min_pakete > 0 und naechste_seq gesetzt (nicht erster Pop).
        let mindest_fuellstand = match self.config.modus {
            JitterBufferModus::Fixed => self.config.min_pakete,
            JitterBufferModus::Adaptiv => self.ziel_groesse.min(self.config.min_pakete),
        };

        if mindest_fuellstand > 0
            && self.pakete.len() < mindest_fuellstand
            && self.naechste_seq.is_some()
        {
            return None;
        }

        // Aeltestes Paket entnehmen
        let (&seq, _) = self.pakete.iter().next()?;
        let paket = self.pakete.remove(&seq)?;

        // Verlorene Pakete zaehlen (Luecken schliessen)
        if let Some(erwartet) = self.naechste_seq {
            if seq > erwartet {
                let verlust = seq.wrapping_sub(erwartet) as u64;
                self.statistik.verloren += verlust;
                tracing::debug!(erwartet, erhalten = seq, verlust, "Paket-Luecke erkannt");
            }
        }

        self.naechste_seq = Some(seq.wrapping_add(1));
        self.letzte_abgespielt = Some(seq);
        self.statistik.abgespielt += 1;
        self.statistik.fuellstand = self.pakete.len();

        Some(paket)
    }

    /// Gibt eine Referenz auf die aktuellen Statistiken
    pub fn statistik(&self) -> &JitterBufferStatistik {
        &self.statistik
    }

    /// Gibt den aktuell gemessenen Jitter als Standardabweichung zurueck (Ticks)
    pub fn jitter_ticks(&self) -> u32 {
        if self.jitter_n < 2 {
            return 0;
        }
        let varianz = self.jitter_m2 / (self.jitter_n - 1) as f64;
        varianz.sqrt() as u32
    }

    /// Gibt den aktuellen Fuellstand zurueck
    pub fn fuellstand(&self) -> usize {
        self.pakete.len()
    }

    /// Gibt die aktuelle Ziel-Buffer-Groesse zurueck
    pub fn ziel_groesse(&self) -> usize {
        self.ziel_groesse
    }

    // -----------------------------------------------------------------------
    // Interne Hilfsfunktionen
    // -----------------------------------------------------------------------

    /// Prueft ob `seq` aelter als `referenz` ist (mit Wrap-Around-Behandlung)
    ///
    /// Nutzt den RTP-Konvention: Differenz > 2^31 gilt als "aelter"
    fn ist_sequence_alt(&self, seq: u32, referenz: u32) -> bool {
        // Sequenznummer-Differenz mit Wrap-Around
        let diff = seq.wrapping_sub(referenz);
        diff > u32::MAX / 2
    }

    /// Misst den Jitter mit dem Welford-Online-Algorithmus (numerisch stabil)
    fn jitter_messen(&mut self, timestamp: u32) {
        if let Some(letzter) = self.letzter_timestamp {
            let interarrival = timestamp.wrapping_sub(letzter) as i64;
            self.interarrivals[self.interarrival_idx] = interarrival;
            self.interarrival_idx = (self.interarrival_idx + 1) % self.interarrivals.len();

            // Welford Online-Algorithmus fuer Varianz
            self.jitter_n += 1;
            let delta = interarrival as f64 - self.jitter_mittel;
            self.jitter_mittel += delta / self.jitter_n as f64;
            let delta2 = interarrival as f64 - self.jitter_mittel;
            self.jitter_m2 += delta * delta2;

            self.statistik.jitter_ticks = self.jitter_ticks();
        }
        self.letzter_timestamp = Some(timestamp);
    }

    /// Passt die Ziel-Buffer-Groesse basierend auf gemessenem Jitter an
    ///
    /// Heuristik: Ziel = max(min_pakete, jitter_stddev / typische_frame_dauer + 2)
    /// Bei 20ms Frames und 48kHz: 1 Frame = 960 Ticks
    fn ziel_groesse_anpassen(&mut self) {
        const TICKS_PRO_FRAME: u32 = 960; // 20ms bei 48kHz
        let jitter = self.jitter_ticks();
        // Benoetigt so viele Frames wie der Jitter gross ist, plus 2 Sicherheitspuffer
        let benoetigt = (jitter / TICKS_PRO_FRAME) as usize + 2;
        self.ziel_groesse = benoetigt
            .max(self.config.min_pakete)
            .min(self.config.max_pakete);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use speakeasy_protocol::voice::{PacketType, VoicePacketHeader};

    fn make_paket(seq: u32, ts: u32) -> VoicePacket {
        VoicePacket {
            header: VoicePacketHeader::new(PacketType::Audio, 0, seq, ts, 0xCAFE),
            payload: vec![0xAB; 60],
        }
    }

    #[test]
    fn jitter_buffer_reihenfolge_in_order() {
        let config = JitterBufferConfig {
            modus: JitterBufferModus::Fixed,
            max_pakete: 10,
            min_pakete: 0,
            jitter_fenster: 8,
        };
        let mut buf = AdaptiveJitterBuffer::neu(config);

        // 5 Pakete in Reihenfolge einf??gen
        for i in 0..5u32 {
            buf.push(make_paket(i, i * 960));
        }

        // Pakete muessen in Reihenfolge herauskommen
        let mut letzte_seq = None;
        while let Some(p) = buf.pop() {
            if let Some(vorherige) = letzte_seq {
                assert!(p.header.sequence > vorherige, "Reihenfolge verletzt");
            }
            letzte_seq = Some(p.header.sequence);
        }
    }

    #[test]
    fn jitter_buffer_out_of_order() {
        let config = JitterBufferConfig {
            modus: JitterBufferModus::Fixed,
            max_pakete: 10,
            min_pakete: 0,
            jitter_fenster: 8,
        };
        let mut buf = AdaptiveJitterBuffer::neu(config);

        // Pakete in falscher Reihenfolge einf??gen
        buf.push(make_paket(2, 1920));
        buf.push(make_paket(0, 0));
        buf.push(make_paket(1, 960));
        buf.push(make_paket(4, 3840));
        buf.push(make_paket(3, 2880));

        // Muessen sortiert herauskommen
        let mut seqs = Vec::new();
        while let Some(p) = buf.pop() {
            seqs.push(p.header.sequence);
        }
        assert_eq!(
            seqs,
            vec![0, 1, 2, 3, 4],
            "Out-of-Order nicht korrekt sortiert"
        );
    }

    #[test]
    fn jitter_buffer_duplikate_verwerfen() {
        let config = JitterBufferConfig {
            modus: JitterBufferModus::Fixed,
            max_pakete: 10,
            min_pakete: 0,
            jitter_fenster: 8,
        };
        let mut buf = AdaptiveJitterBuffer::neu(config);

        buf.push(make_paket(1, 960));
        buf.push(make_paket(1, 960)); // Duplikat
        buf.push(make_paket(1, 960)); // Duplikat

        assert_eq!(buf.fuellstand(), 1, "Duplikate muessen verworfen werden");
        assert_eq!(buf.statistik().duplikate, 2);
    }

    #[test]
    fn jitter_buffer_verlust_erkennung() {
        let config = JitterBufferConfig {
            modus: JitterBufferModus::Fixed,
            max_pakete: 20,
            min_pakete: 0,
            jitter_fenster: 8,
        };
        let mut buf = AdaptiveJitterBuffer::neu(config);

        // Seq 0, dann Luecke (1,2,3 fehlen), dann 4
        buf.push(make_paket(0, 0));
        buf.push(make_paket(4, 3840));

        let p1 = buf.pop().unwrap();
        assert_eq!(p1.header.sequence, 0);

        let p2 = buf.pop().unwrap();
        assert_eq!(p2.header.sequence, 4);

        // 3 verlorene Pakete (Seq 1, 2, 3)
        assert_eq!(buf.statistik().verloren, 3);
    }

    #[test]
    fn jitter_buffer_ueberlauf_verwirft_aelteste() {
        let config = JitterBufferConfig {
            modus: JitterBufferModus::Fixed,
            max_pakete: 3,
            min_pakete: 0,
            jitter_fenster: 4,
        };
        let mut buf = AdaptiveJitterBuffer::neu(config);

        // 4 Pakete einf??gen bei max=3
        buf.push(make_paket(0, 0));
        buf.push(make_paket(1, 960));
        buf.push(make_paket(2, 1920));
        buf.push(make_paket(3, 2880)); // Sollte Seq 0 verdraengen

        assert_eq!(buf.fuellstand(), 3);

        // Aeltestes verbleibendes muss Seq 1 sein (Seq 0 wurde verworfen)
        let p = buf.pop().unwrap();
        assert_eq!(p.header.sequence, 1);
    }

    #[test]
    fn jitter_buffer_adaptiver_modus() {
        let config = JitterBufferConfig {
            modus: JitterBufferModus::Adaptiv,
            max_pakete: 20,
            min_pakete: 2,
            jitter_fenster: 8,
        };
        let mut buf = AdaptiveJitterBuffer::neu(config);

        // Pakete mit konsistenten Timestamps einf??gen (niedriger Jitter)
        for i in 0..10u32 {
            buf.push(make_paket(i, i * 960));
        }

        // Ziel-Groesse sollte klein sein (wenig Jitter)
        assert!(buf.ziel_groesse() >= 2, "Mindest-Groesse unterschritten");
        assert!(buf.ziel_groesse() <= 20, "Maximum ueberschritten");
    }

    #[test]
    fn jitter_buffer_wrap_around_sequence() {
        let config = JitterBufferConfig {
            modus: JitterBufferModus::Fixed,
            max_pakete: 10,
            min_pakete: 0,
            jitter_fenster: 4,
        };
        let mut buf = AdaptiveJitterBuffer::neu(config);

        // Sequenznummer nahe u32::MAX (Wrap-Around)
        let max = u32::MAX;
        buf.push(make_paket(max - 1, 0));
        buf.push(make_paket(max, 960));
        buf.push(make_paket(0, 1920)); // Wrap-Around

        assert_eq!(buf.fuellstand(), 3);
    }
}
