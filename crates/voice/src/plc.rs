//! Packet Loss Concealment (PLC)
//!
//! Erkennt fehlende Pakete (Luecken in der Sequenznummer) und fuegt
//! Ersatz-Audio ein, um hoerbare Artefakte zu minimieren.
//!
//! ## Strategien
//! 1. **FEC-Dekodierung**: Wenn das aktuelle Paket FEC-Daten vom vorherigen Paket
//!    enthaelt (Opus In-Band FEC), wird das verlorene Paket rekonstruiert.
//! 2. **Wiederholung mit Fade**: Das letzte gueltige Paket wird wiederholt,
//!    aber mit abnehmendem Lautstaerkepegel (Fade-Out).
//! 3. **Stille**: Nach mehreren aufeinanderfolgenden Verlusten wird Stille eingefuegt.
//!
//! ## Grenzen
//! - Maximale Wiederholungen vor Stille: `MAX_WIEDERHOLUNGEN`
//! - Fade-Faktor pro Wiederholung: `FADE_FAKTOR`

use speakeasy_protocol::voice::{PacketType, VoiceFlags, VoicePacket, VoicePacketHeader};

/// Maximale Anzahl von Paket-Wiederholungen vor dem Uebergang zu Stille
pub const MAX_WIEDERHOLUNGEN: u32 = 3;

/// Fade-Faktor pro Wiederholung (0.0–1.0, 0.75 = 25% Abnahme pro Frame)
pub const FADE_FAKTOR: f32 = 0.75;

// ---------------------------------------------------------------------------
// PLC-Ergebnis
// ---------------------------------------------------------------------------

/// Ergebnis einer PLC-Operation
#[derive(Debug, Clone)]
pub enum PlcErgebnis {
    /// Paket war nicht verloren – Original zurueck
    Original(VoicePacket),
    /// Verlust durch FEC-Daten aus nachfolgendem Paket rekonstruiert
    FecRekonstruiert(VoicePacket),
    /// Verlust durch Wiederholung des letzten Pakets (mit Fade) verdeckt
    Wiederholung(VoicePacket),
    /// Zu viele aufeinanderfolgende Verluste – Stille eingefuegt
    Stille(VoicePacket),
    /// Kein letztes Paket verfuegbar – leeres Ergebnis
    Leer,
}

impl PlcErgebnis {
    /// Gibt das enthaltene Paket zurueck (falls vorhanden)
    pub fn paket(&self) -> Option<&VoicePacket> {
        match self {
            Self::Original(p)
            | Self::FecRekonstruiert(p)
            | Self::Wiederholung(p)
            | Self::Stille(p) => Some(p),
            Self::Leer => None,
        }
    }

    /// Prueft ob es ein Original-Paket ist
    pub fn ist_original(&self) -> bool {
        matches!(self, Self::Original(_))
    }

    /// Prueft ob PLC aktiv war
    pub fn plc_aktiv(&self) -> bool {
        !matches!(self, Self::Original(_) | Self::Leer)
    }
}

// ---------------------------------------------------------------------------
// Statistiken
// ---------------------------------------------------------------------------

/// PLC-Statistiken
#[derive(Debug, Clone, Default)]
pub struct PlcStatistik {
    /// Originale Pakete (kein Verlust)
    pub originale: u64,
    /// FEC-rekonstruierte Pakete
    pub fec_rekonstruiert: u64,
    /// Wiederholte Pakete
    pub wiederholungen: u64,
    /// Stille-Pakete eingefuegt
    pub stille_eingefuegt: u64,
    /// Gesamte verlorene Pakete (inkl. aller PLC-Arten)
    pub gesamt_verloren: u64,
}

impl PlcStatistik {
    /// Verlustrate (0.0–1.0)
    pub fn verlust_rate(&self) -> f64 {
        let gesamt = self.originale + self.gesamt_verloren;
        if gesamt == 0 {
            0.0
        } else {
            self.gesamt_verloren as f64 / gesamt as f64
        }
    }
}

// ---------------------------------------------------------------------------
// PacketLossConcealer
// ---------------------------------------------------------------------------

/// Packet Loss Concealer – verdeckt Paketverluste fuer den Wiedergabe-Pfad
///
/// Wird pro Client im Empfangs-Pfad verwendet (nach dem Jitter Buffer).
/// Nicht thread-safe (single-threaded per Client).
pub struct PacketLossConcealer {
    /// Letztes erfolgreich empfangenes Paket (fuer Wiederholung)
    letztes_paket: Option<VoicePacket>,
    /// Anzahl aufeinanderfolgender Verluste
    aufeinanderfolgende_verluste: u32,
    /// Naechste erwartete Sequenznummer
    naechste_seq: Option<u32>,
    /// Aktueller Fade-Faktor (wird bei jedem Verlust reduziert)
    aktueller_fade: f32,
    /// Statistiken
    statistik: PlcStatistik,
}

impl PacketLossConcealer {
    /// Erstellt einen neuen PLC
    pub fn neu() -> Self {
        Self {
            letztes_paket: None,
            aufeinanderfolgende_verluste: 0,
            naechste_seq: None,
            aktueller_fade: 1.0,
            statistik: PlcStatistik::default(),
        }
    }

    /// Verarbeitet ein eingehendes Paket
    ///
    /// Falls Luecken in der Sequenznummer erkannt werden, werden PLC-Pakete
    /// fuer die fehlenden Sequenznummern generiert (via Iterator/Vec).
    ///
    /// Gibt eine Liste von Paketen zurueck:
    /// - Bei keinem Verlust: `[Original]`
    /// - Bei N verlorenen Paketen: `[PLC_1, PLC_2, ..., PLC_N, Original]`
    pub fn verarbeiten(&mut self, paket: VoicePacket) -> Vec<PlcErgebnis> {
        let seq = paket.header.sequence;
        let mut ergebnisse = Vec::new();

        if let Some(erwartet) = self.naechste_seq {
            if seq > erwartet {
                // Luecke erkannt – fehlende Pakete durch PLC ersetzen
                let luecke = seq.wrapping_sub(erwartet).min(MAX_WIEDERHOLUNGEN + 1);
                for missing_seq in erwartet..erwartet.wrapping_add(luecke) {
                    // FEC pruefen: Hat das aktuelle Paket FEC-Daten fuer den Vorgaenger?
                    let fec_verfuegbar = paket.header.hat_flag(VoiceFlags::FEC)
                        && missing_seq == seq.wrapping_sub(1);

                    let plc_ergebnis = if fec_verfuegbar {
                        self.fec_rekonstruieren(missing_seq, &paket)
                    } else {
                        self.verlust_verdecken(missing_seq, paket.header.timestamp)
                    };

                    self.statistik.gesamt_verloren += 1;
                    ergebnisse.push(plc_ergebnis);
                }
            } else if seq < erwartet {
                // Spaetes/Duplikat-Paket – ignorieren
                tracing::debug!(seq, erwartet, "PLC: Spaetes Paket ignoriert");
                return ergebnisse;
            }
        }

        // Verlust-Kette zuruecksetzen
        self.aufeinanderfolgende_verluste = 0;
        self.aktueller_fade = 1.0;

        self.naechste_seq = Some(seq.wrapping_add(1));
        self.letztes_paket = Some(paket.clone());
        self.statistik.originale += 1;
        ergebnisse.push(PlcErgebnis::Original(paket));
        ergebnisse
    }

    /// Gibt die aktuellen Statistiken zurueck
    pub fn statistik(&self) -> &PlcStatistik {
        &self.statistik
    }

    // -----------------------------------------------------------------------
    // Interne PLC-Strategien
    // -----------------------------------------------------------------------

    /// Rekonstruiert ein verlorenes Paket aus FEC-Daten des Nachfolgers
    ///
    /// Opus In-Band FEC: Das FEC-Daten-Feld des aktuellen Pakets enthaelt
    /// eine niedrig-bitrate Kopie des vorherigen Frames.
    /// Da wir keinen Opus-Decoder haben (Server leitet nur weiter), markieren
    /// wir das Paket als FEC-rekonstruiert und nutzen die rohen FEC-Bytes.
    fn fec_rekonstruieren(&mut self, seq: u32, nachfolger: &VoicePacket) -> PlcErgebnis {
        // FEC-Bytes sind im Payload des Nachfolgers kodiert (Opus-spezifisch).
        // Wir erstellen ein neues Paket mit dem Typ Fec und den Payload-Bytes.
        let fec_paket = VoicePacket {
            header: VoicePacketHeader::new(
                PacketType::Fec,
                VoiceFlags::FEC,
                seq,
                nachfolger.header.timestamp.wrapping_sub(960), // 20ms zurueck bei 48kHz
                nachfolger.header.ssrc,
            ),
            payload: nachfolger.payload.clone(), // FEC-Bytes aus Nachfolger
        };

        self.statistik.fec_rekonstruiert += 1;
        tracing::debug!(seq, "PLC: FEC-Rekonstruktion");
        PlcErgebnis::FecRekonstruiert(fec_paket)
    }

    /// Verdeckt einen Verlust durch Wiederholung oder Stille
    fn verlust_verdecken(&mut self, seq: u32, referenz_timestamp: u32) -> PlcErgebnis {
        self.aufeinanderfolgende_verluste += 1;

        if self.aufeinanderfolgende_verluste > MAX_WIEDERHOLUNGEN {
            // Zu viele Verluste – Stille einfuegen
            self.statistik.stille_eingefuegt += 1;
            tracing::debug!(seq, "PLC: Stille eingefuegt");
            return PlcErgebnis::Stille(self.stille_paket(seq, referenz_timestamp));
        }

        if let Some(ref letztes) = self.letztes_paket.clone() {
            // Letztes Paket wiederholen mit Fade-Out
            self.aktueller_fade *= FADE_FAKTOR;
            let fade = self.aktueller_fade;

            let mut payload = letztes.payload.clone();
            // Amplitude reduzieren: Bytes skalieren (vereinfachte Implementierung)
            // In einer echten Implementierung wuerden PCM-Samples skaliert werden.
            // Da wir Opus-Bytes haben (nicht PCM), simulieren wir den Fade durch
            // Payload-Verkuerzung (niedrigere Bitrate = leiser klingendes Ersatz-Paket).
            let neue_laenge = ((payload.len() as f32 * fade) as usize).max(1);
            payload.truncate(neue_laenge);

            let wiederholungs_paket = VoicePacket {
                header: VoicePacketHeader::new(
                    PacketType::Audio,
                    0,
                    seq,
                    referenz_timestamp.wrapping_sub(960),
                    letztes.header.ssrc,
                ),
                payload,
            };

            self.statistik.wiederholungen += 1;
            tracing::debug!(seq, fade, "PLC: Wiederholung mit Fade");
            PlcErgebnis::Wiederholung(wiederholungs_paket)
        } else {
            // Kein letztes Paket verfuegbar
            PlcErgebnis::Leer
        }
    }

    /// Erstellt ein Stille-Paket fuer die gegebene Sequenznummer
    fn stille_paket(&self, seq: u32, referenz_timestamp: u32) -> VoicePacket {
        let ssrc = self
            .letztes_paket
            .as_ref()
            .map(|p| p.header.ssrc)
            .unwrap_or(0);
        VoicePacket::neu_silence(seq, referenz_timestamp.wrapping_sub(960), ssrc)
    }
}

impl Default for PacketLossConcealer {
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
    use speakeasy_protocol::voice::VoicePacketHeader;

    fn make_paket(seq: u32, ssrc: u32) -> VoicePacket {
        VoicePacket {
            header: VoicePacketHeader::new(PacketType::Audio, 0, seq, seq * 960, ssrc),
            payload: vec![0xAB; 80],
        }
    }

    fn make_fec_paket(seq: u32, ssrc: u32) -> VoicePacket {
        VoicePacket {
            header: VoicePacketHeader::new(
                PacketType::Audio,
                VoiceFlags::FEC,
                seq,
                seq * 960,
                ssrc,
            ),
            payload: vec![0xCD; 40],
        }
    }

    #[test]
    fn plc_kein_verlust_original_durchgereicht() {
        let mut plc = PacketLossConcealer::neu();

        let ergebnisse = plc.verarbeiten(make_paket(0, 0xCAFE));
        assert_eq!(ergebnisse.len(), 1);
        assert!(ergebnisse[0].ist_original());

        let ergebnisse = plc.verarbeiten(make_paket(1, 0xCAFE));
        assert_eq!(ergebnisse.len(), 1);
        assert!(ergebnisse[0].ist_original());

        assert_eq!(plc.statistik().originale, 2);
        assert_eq!(plc.statistik().gesamt_verloren, 0);
    }

    #[test]
    fn plc_einzelverlust_wiederholung() {
        let mut plc = PacketLossConcealer::neu();

        // Seq 0 korrekt
        plc.verarbeiten(make_paket(0, 0xCAFE));

        // Seq 1 fehlt, Seq 2 kommt an
        let ergebnisse = plc.verarbeiten(make_paket(2, 0xCAFE));

        // Ergebnisse: [PLC fuer Seq1, Original Seq2]
        assert_eq!(ergebnisse.len(), 2, "Erwartet PLC + Original");
        assert!(ergebnisse[0].plc_aktiv(), "Erstes Ergebnis muss PLC sein");
        assert!(
            ergebnisse[1].ist_original(),
            "Zweites Ergebnis muss Original sein"
        );

        assert_eq!(plc.statistik().wiederholungen, 1);
        assert_eq!(plc.statistik().gesamt_verloren, 1);
    }

    #[test]
    fn plc_mehrere_verluste_fade() {
        let mut plc = PacketLossConcealer::neu();

        plc.verarbeiten(make_paket(0, 0xCAFE));

        // Seq 1, 2, 3 fehlen – 4 kommt
        let ergebnisse = plc.verarbeiten(make_paket(4, 0xCAFE));

        // 3 Luecken (begrenzt auf MAX_WIEDERHOLUNGEN+1=4, aber Luecke=3)
        assert_eq!(ergebnisse.len(), 4); // 3 PLC + 1 Original

        // Alle PLC-Eintraege
        for e in &ergebnisse[..3] {
            assert!(e.plc_aktiv());
        }
        assert!(ergebnisse[3].ist_original());
    }

    #[test]
    fn plc_zu_viele_verluste_stille() {
        let mut plc = PacketLossConcealer::neu();

        plc.verarbeiten(make_paket(0, 0xCAFE));

        // MAX_WIEDERHOLUNGEN + 2 Verluste
        let grosse_luecke = MAX_WIEDERHOLUNGEN + 2;
        let ergebnisse = plc.verarbeiten(make_paket(grosse_luecke + 1, 0xCAFE));

        // Nach MAX_WIEDERHOLUNGEN Wiederholungen -> Stille
        let stille_count = ergebnisse
            .iter()
            .filter(|e| matches!(e, PlcErgebnis::Stille(_)))
            .count();
        assert!(
            stille_count > 0,
            "Stille muss nach vielen Verlusten eingefuegt werden"
        );

        assert!(plc.statistik().stille_eingefuegt > 0);
    }

    #[test]
    fn plc_fec_rekonstruktion() {
        let mut plc = PacketLossConcealer::neu();

        plc.verarbeiten(make_paket(0, 0xCAFE));

        // Seq 1 fehlt; Seq 2 kommt mit FEC-Flag (enthaelt Daten fuer Seq 1)
        let ergebnisse = plc.verarbeiten(make_fec_paket(2, 0xCAFE));

        assert_eq!(ergebnisse.len(), 2);
        assert!(
            matches!(ergebnisse[0], PlcErgebnis::FecRekonstruiert(_)),
            "FEC-Rekonstruktion erwartet, bekam: {:?}",
            ergebnisse[0]
        );
        assert_eq!(plc.statistik().fec_rekonstruiert, 1);
    }

    #[test]
    fn plc_reset_nach_original() {
        let mut plc = PacketLossConcealer::neu();

        plc.verarbeiten(make_paket(0, 0xCAFE));

        // 2 Verluste
        plc.verarbeiten(make_paket(3, 0xCAFE));

        // Danach normale Pakete – aufeinanderfolgende_verluste muss 0 sein
        plc.verarbeiten(make_paket(4, 0xCAFE));
        plc.verarbeiten(make_paket(5, 0xCAFE));

        // Weiterer einzelner Verlust – muss als Wiederholung behandelt werden (nicht Stille)
        let ergebnisse = plc.verarbeiten(make_paket(7, 0xCAFE));
        assert_eq!(ergebnisse.len(), 2);
        assert!(
            matches!(ergebnisse[0], PlcErgebnis::Wiederholung(_)),
            "Nach Reset soll Wiederholung kommen, nicht Stille"
        );
    }

    #[test]
    fn plc_verlust_rate_korrekt() {
        let mut plc = PacketLossConcealer::neu();

        // 5 Originale, dann Luecke mit 1 Verlust
        for i in 0..5u32 {
            plc.verarbeiten(make_paket(i, 1));
        }
        plc.verarbeiten(make_paket(6, 1)); // Seq 5 fehlt

        let rate = plc.statistik().verlust_rate();
        // 1 verloren von 6 gesamt
        assert!(
            (rate - (1.0 / 7.0)).abs() < 0.01,
            "Verlust-Rate falsch: {}",
            rate
        );
    }

    #[test]
    fn plc_stille_paket_hat_dtx_flag() {
        let mut plc = PacketLossConcealer::neu();
        plc.verarbeiten(make_paket(0, 0xCAFE));

        // Grosse Luecke erzwingen (mehr als MAX_WIEDERHOLUNGEN)
        let ergebnisse = plc.verarbeiten(make_paket(MAX_WIEDERHOLUNGEN + 2, 0xCAFE));

        for e in &ergebnisse {
            if let PlcErgebnis::Stille(p) = e {
                assert!(
                    p.header.hat_flag(VoiceFlags::DTX),
                    "Stille-Paket muss DTX-Flag haben"
                );
            }
        }
    }
}
