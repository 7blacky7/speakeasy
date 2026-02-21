//! Voice Activity Detection (VAD)
//!
//! Energie-basierte VAD kombiniert mit Zero-Crossing-Rate.
//! Erkennt ob im aktuellen Frame Sprache vorhanden ist.

use super::AudioProcessor;

/// Konfiguration fuer die VAD
#[derive(Debug, Clone)]
pub struct VadConfig {
    /// Energie-Schwellenwert (normalisiert, 0.0..1.0)
    pub energy_threshold: f32,
    /// Zero-Crossing-Rate Schwellenwert (Anzahl Nulldurchgaenge pro Frame / Frame-Laenge)
    pub zcr_threshold: f32,
    /// Hangover-Zeit: Frames die nach letzter Aktivitaet noch als aktiv gelten
    pub hangover_frames: u32,
    /// Glaettungsfaktor fuer Energie (0.0 = keine Glaettung, 1.0 = volle Glaettung)
    pub smoothing: f32,
}

impl Default for VadConfig {
    fn default() -> Self {
        Self {
            energy_threshold: 0.001,
            zcr_threshold: 0.3,
            hangover_frames: 8,
            smoothing: 0.9,
        }
    }
}

/// Voice Activity Detector
pub struct Vad {
    config: VadConfig,
    smoothed_energy: f32,
    hangover_counter: u32,
    voice_active: bool,
    enabled: bool,
}

impl Vad {
    pub fn new(config: VadConfig) -> Self {
        Self {
            config,
            smoothed_energy: 0.0,
            hangover_counter: 0,
            voice_active: false,
            enabled: true,
        }
    }

    /// Gibt zurueck ob im letzten verarbeiteten Frame Sprache erkannt wurde
    pub fn is_voice_active(&self) -> bool {
        self.voice_active
    }

    /// Analysiert einen Frame und gibt zurueck ob Sprache aktiv ist.
    /// Veraendert die Samples NICHT (VAD ist rein analytisch).
    pub fn detect(&mut self, samples: &[f32]) -> bool {
        if !self.enabled || samples.is_empty() {
            return false;
        }

        let energy = rms_energy(samples);
        // Exponentielle Glaettung der Energie
        self.smoothed_energy =
            self.config.smoothing * self.smoothed_energy + (1.0 - self.config.smoothing) * energy;

        let zcr = zero_crossing_rate(samples);

        let energy_active = self.smoothed_energy > self.config.energy_threshold;
        // Sprache hat typisch moderate ZCR (nicht zu hoch wie Rauschen, nicht 0)
        let zcr_plausible = zcr < self.config.zcr_threshold;

        if energy_active && zcr_plausible {
            self.hangover_counter = self.config.hangover_frames;
            self.voice_active = true;
        } else if self.hangover_counter > 0 {
            self.hangover_counter -= 1;
            self.voice_active = true;
        } else {
            self.voice_active = false;
        }

        self.voice_active
    }

    /// Setzt den Energie-Schwellenwert
    pub fn set_energy_threshold(&mut self, threshold: f32) {
        self.config.energy_threshold = threshold;
    }

    /// Gibt die geglaettete Energie zurueck (nuetzlich fuer Kalibrierung)
    pub fn smoothed_energy(&self) -> f32 {
        self.smoothed_energy
    }
}

impl AudioProcessor for Vad {
    /// VAD veraendert keine Samples - nur interne Zustandsaktualisierung
    fn process(&mut self, samples: &mut [f32]) {
        self.detect(samples);
    }

    fn reset(&mut self) {
        self.smoothed_energy = 0.0;
        self.hangover_counter = 0;
        self.voice_active = false;
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
}

/// Berechnet den RMS-Energiewert eines Frames
pub fn rms_energy(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = samples.iter().map(|s| s * s).sum();
    (sum_sq / samples.len() as f32).sqrt()
}

/// Berechnet die normalisierte Zero-Crossing-Rate
pub fn zero_crossing_rate(samples: &[f32]) -> f32 {
    if samples.len() < 2 {
        return 0.0;
    }
    let crossings = samples
        .windows(2)
        .filter(|w| (w[0] >= 0.0) != (w[1] >= 0.0))
        .count();
    crossings as f32 / (samples.len() - 1) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vad_stille_nicht_aktiv() {
        let mut vad = Vad::new(VadConfig::default());
        // Absolut stilles Signal
        let samples = vec![0.0f32; 480];
        let active = vad.detect(&samples);
        assert!(!active, "Stille sollte keine Sprachaktivitaet zeigen");
    }

    #[test]
    fn vad_rauschen_erkannt() {
        let mut vad = Vad::new(VadConfig {
            energy_threshold: 0.0001,
            smoothing: 0.0, // keine Glaettung fuer sofortige Reaktion
            ..VadConfig::default()
        });
        // Mittleres Signal
        let samples = vec![0.1f32; 480];
        let active = vad.detect(&samples);
        assert!(active, "Mittleres Signal sollte erkannt werden");
    }

    #[test]
    fn vad_hangover_haelt_aktiv() {
        let config = VadConfig {
            energy_threshold: 0.001,
            hangover_frames: 3,
            smoothing: 0.0,
            ..VadConfig::default()
        };
        let mut vad = Vad::new(config);

        // Erst aktiv machen
        let lautes_signal = vec![0.5f32; 480];
        vad.detect(&lautes_signal);
        assert!(vad.is_voice_active());

        // Dann still - Hangover haelt 3 Frames aktiv
        let stille = vec![0.0f32; 480];
        assert!(vad.detect(&stille), "Hangover Frame 1");
        assert!(vad.detect(&stille), "Hangover Frame 2");
        assert!(vad.detect(&stille), "Hangover Frame 3");
        assert!(!vad.detect(&stille), "Nach Hangover sollte inaktiv sein");
    }

    #[test]
    fn vad_reset_setzt_zustand() {
        let mut vad = Vad::new(VadConfig::default());
        let samples = vec![0.5f32; 480];
        vad.detect(&samples);
        vad.reset();
        assert_eq!(vad.smoothed_energy(), 0.0);
        assert!(!vad.is_voice_active());
    }

    #[test]
    fn rms_energy_null_fuer_stille() {
        let samples = vec![0.0f32; 480];
        assert_eq!(rms_energy(&samples), 0.0);
    }

    #[test]
    fn rms_energy_korrekt() {
        // RMS von [1, 1, 1, 1] = 1.0
        let samples = vec![1.0f32; 4];
        assert!((rms_energy(&samples) - 1.0).abs() < 0.001);

        // RMS von [0.5, 0.5, 0.5, 0.5] = 0.5
        let samples = vec![0.5f32; 4];
        assert!((rms_energy(&samples) - 0.5).abs() < 0.001);
    }

    #[test]
    fn zcr_sinus_niedrig() {
        // Ein Sinus mit wenigen Nulldurchgaengen
        let samples: Vec<f32> = (0..480)
            .map(|i| (i as f32 * 0.05).sin())
            .collect();
        let zcr = zero_crossing_rate(&samples);
        // Sinus mit niedriger Frequenz hat niedrige ZCR
        assert!(zcr < 0.1, "Niederfrequenter Sinus hat niedrige ZCR: {}", zcr);
    }

    #[test]
    fn vad_process_trait_unveraendert() {
        let mut vad = Vad::new(VadConfig::default());
        let original = vec![0.5f32; 480];
        let mut samples = original.clone();
        vad.process(&mut samples);
        // VAD veraendert keine Samples
        assert_eq!(samples, original);
    }
}
