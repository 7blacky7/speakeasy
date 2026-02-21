//! De-Esser
//!
//! Reduziert unangenehme Zischlaute (S, Sch) im Bereich 4-8 kHz.
//! Frequenzband-basiert: Energie im Hochfrequenzbereich wird detektiert
//! und bei Ueberschreitung des Schwellenwerts reduziert.

use super::AudioProcessor;

/// Konfiguration fuer den De-Esser
#[derive(Debug, Clone)]
pub struct DeEsserConfig {
    /// Schwellenwert (RMS des Hochfrequenzanteils, normalisiert)
    pub threshold: f32,
    /// Kompressionsverhaeltnis (1.0 = kein Effekt, 4.0 = starke Kompression)
    pub ratio: f32,
    /// Glaettungsfaktor fuer Pegeldetektierung
    pub smoothing: f32,
    /// Abtastrate in Hz (benoetigt fuer Filterkoeffizienten)
    pub sample_rate: f32,
}

impl Default for DeEsserConfig {
    fn default() -> Self {
        Self {
            threshold: 0.05,
            ratio: 3.0,
            smoothing: 0.9,
            sample_rate: 48000.0,
        }
    }
}

/// De-Esser Prozessor
///
/// Arbeitsweise: Ein einfacher Hochpassfilter (1. Ordnung) extrahiert
/// den Hochfrequenzanteil. Dessen RMS-Pegel wird gemessen. Bei
/// Ueberschreitung des Schwellenwerts wird ein Gain-Faktor berechnet
/// und auf das Vollband-Signal angewendet.
pub struct DeEsser {
    config: DeEsserConfig,
    /// Hochpass-Filterkoeffizient
    hp_coeff: f32,
    /// Letzter HP-Ausgangswert (Filterstate)
    hp_last_out: f32,
    /// Letzter Eingangswert (Filterstate)
    hp_last_in: f32,
    /// Geglaettete Energie des HF-Anteils
    smoothed_hf_energy: f32,
    enabled: bool,
}

impl DeEsser {
    pub fn new(config: DeEsserConfig) -> Self {
        // RC-Hochpass: fc = 4000 Hz
        // alpha = RC / (RC + dt) = 1 / (1 + 2*pi*fc/fs)
        let fc = 4000.0;
        let hp_coeff = 1.0 / (1.0 + 2.0 * std::f32::consts::PI * fc / config.sample_rate);

        Self {
            hp_coeff,
            hp_last_out: 0.0,
            hp_last_in: 0.0,
            smoothed_hf_energy: 0.0,
            config,
            enabled: true,
        }
    }

    /// Setzt Threshold und Ratio zur Laufzeit
    pub fn set_threshold(&mut self, threshold: f32) {
        self.config.threshold = threshold;
    }

    pub fn set_ratio(&mut self, ratio: f32) {
        self.config.ratio = ratio.max(1.0);
    }
}

impl AudioProcessor for DeEsser {
    fn process(&mut self, samples: &mut [f32]) {
        if !self.enabled {
            return;
        }

        // 1. Hochpassfilter anwenden um HF-Anteil zu extrahieren
        let mut hf_samples = Vec::with_capacity(samples.len());
        for &x in samples.iter() {
            // Einfacher RC-Hochpass: y[n] = alpha * (y[n-1] + x[n] - x[n-1])
            let y = self.hp_coeff * (self.hp_last_out + x - self.hp_last_in);
            self.hp_last_out = y;
            self.hp_last_in = x;
            hf_samples.push(y);
        }

        // 2. RMS-Energie des HF-Anteils messen
        let hf_rms = {
            let sum_sq: f32 = hf_samples.iter().map(|s| s * s).sum();
            (sum_sq / hf_samples.len() as f32).sqrt()
        };

        // 3. Energie glaetten
        self.smoothed_hf_energy = self.config.smoothing * self.smoothed_hf_energy
            + (1.0 - self.config.smoothing) * hf_rms;

        // 4. Gain berechnen wenn Threshold ueberschritten
        if self.smoothed_hf_energy > self.config.threshold {
            let excess = self.smoothed_hf_energy / self.config.threshold;
            // Gain-Reduktion: gain = 1 / (1 + (excess-1) * (1 - 1/ratio))
            let reduction = 1.0 - 1.0 / self.config.ratio;
            let gain = 1.0 / (1.0 + (excess - 1.0) * reduction);
            let gain = gain.clamp(1.0 / self.config.ratio, 1.0);

            for sample in samples.iter_mut() {
                *sample *= gain;
            }
        }
    }

    fn reset(&mut self) {
        self.hp_last_out = 0.0;
        self.hp_last_in = 0.0;
        self.smoothed_hf_energy = 0.0;
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deesser_erstellen() {
        let de = DeEsser::new(DeEsserConfig::default());
        assert!(de.is_enabled());
        assert!(de.hp_coeff > 0.0 && de.hp_coeff < 1.0);
    }

    #[test]
    fn deesser_deaktiviert_unveraendert() {
        let mut de = DeEsser::new(DeEsserConfig::default());
        de.set_enabled(false);
        let original = vec![0.5f32; 480];
        let mut samples = original.clone();
        de.process(&mut samples);
        assert_eq!(samples, original);
    }

    #[test]
    fn deesser_reset() {
        let mut de = DeEsser::new(DeEsserConfig::default());
        let mut samples = vec![0.5f32; 480];
        de.process(&mut samples);
        de.reset();
        assert_eq!(de.hp_last_out, 0.0);
        assert_eq!(de.smoothed_hf_energy, 0.0);
    }

    #[test]
    fn deesser_daempft_hochfrequentes_signal() {
        let config = DeEsserConfig {
            threshold: 0.01,
            ratio: 8.0,
            smoothing: 0.0, // sofortige Reaktion
            sample_rate: 48000.0,
        };
        let mut de = DeEsser::new(config);

        // Hochfrequentes Signal (Sinus bei 8kHz)
        let samples_in: Vec<f32> = (0..480)
            .map(|i| (i as f32 * 2.0 * std::f32::consts::PI * 8000.0 / 48000.0).sin() * 0.3)
            .collect();
        let mut samples = samples_in.clone();
        de.process(&mut samples);

        let energy_in: f32 = samples_in.iter().map(|s| s * s).sum::<f32>();
        let energy_out: f32 = samples.iter().map(|s| s * s).sum::<f32>();
        assert!(
            energy_out <= energy_in,
            "De-Esser sollte Energie nicht erhoehen: in={} out={}",
            energy_in,
            energy_out
        );
    }

    #[test]
    fn deesser_niederfrequentes_signal_unveraendert() {
        let config = DeEsserConfig {
            threshold: 0.05,
            ratio: 4.0,
            smoothing: 0.0,
            sample_rate: 48000.0,
        };
        let mut de = DeEsser::new(config);

        // Sehr niederfrequentes Signal (100 Hz) - wenig HF-Energie
        let samples_in: Vec<f32> = (0..480)
            .map(|i| (i as f32 * 2.0 * std::f32::consts::PI * 100.0 / 48000.0).sin() * 0.3)
            .collect();
        let mut samples = samples_in.clone();
        de.process(&mut samples);

        // Bei niedrigem HF-Anteil sollte kein Gain applied werden (Energie bleibt aehnlich)
        let energy_ratio: f32 = samples.iter().map(|s| s * s).sum::<f32>()
            / samples_in.iter().map(|s| s * s).sum::<f32>();
        assert!(
            energy_ratio > 0.8,
            "Niederfrequentes Signal sollte kaum gedaempft werden: ratio={}",
            energy_ratio
        );
    }

    #[test]
    fn deesser_threshold_setzbar() {
        let mut de = DeEsser::new(DeEsserConfig::default());
        de.set_threshold(0.1);
        assert!((de.config.threshold - 0.1).abs() < f32::EPSILON);
    }

    #[test]
    fn deesser_ratio_minimum_eins() {
        let mut de = DeEsser::new(DeEsserConfig::default());
        de.set_ratio(0.1); // Ungueltig - wird auf 1.0 geclamped
        assert!(de.config.ratio >= 1.0);
    }
}
