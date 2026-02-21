//! Automatic Gain Control (AGC)
//!
//! Regelt den Eingangspegel automatisch auf einen Zielwert.
//! Enthaelt Attack/Release-Glaettung und einen Limiter.

use super::AudioProcessor;

/// Konfiguration fuer den AGC
#[derive(Debug, Clone)]
pub struct AgcConfig {
    /// Ziel-RMS-Pegel (normalisiert, z.B. 0.1 fuer ca. -20 dBFS)
    pub target_level: f32,
    /// Maximaler Gain-Faktor
    pub max_gain: f32,
    /// Minimaler Gain-Faktor (verhindert Aufblasen von Stille)
    pub min_gain: f32,
    /// Attack-Koeffizient pro Sample (wie schnell Gain steigt)
    pub attack_coeff: f32,
    /// Release-Koeffizient pro Sample (wie schnell Gain sinkt)
    pub release_coeff: f32,
    /// Limiter-Schwellenwert (Hard Clip, z.B. 0.99)
    pub limiter_threshold: f32,
}

impl AgcConfig {
    /// Erstellt eine Konfiguration fuer Sprachverarbeitung bei gegebener Abtastrate
    pub fn speech(sample_rate: f32) -> Self {
        Self {
            target_level: 0.1,
            max_gain: 20.0,
            min_gain: 0.1,
            attack_coeff: Self::time_to_coeff(0.01, sample_rate),
            release_coeff: Self::time_to_coeff(0.15, sample_rate),
            limiter_threshold: 0.95,
        }
    }

    fn time_to_coeff(time_secs: f32, sample_rate: f32) -> f32 {
        if time_secs <= 0.0 {
            return 0.0;
        }
        (-1.0 / (time_secs * sample_rate)).exp()
    }
}

impl Default for AgcConfig {
    fn default() -> Self {
        Self::speech(48000.0)
    }
}

/// Automatic Gain Control Prozessor
pub struct Agc {
    config: AgcConfig,
    current_gain: f32,
    enabled: bool,
}

impl Agc {
    pub fn new(config: AgcConfig) -> Self {
        Self {
            current_gain: 1.0,
            config,
            enabled: true,
        }
    }

    /// Gibt den aktuellen Gain-Wert zurueck
    pub fn current_gain(&self) -> f32 {
        self.current_gain
    }

    /// Setzt den Ziel-Pegel
    pub fn set_target_level(&mut self, level: f32) {
        self.config.target_level = level.clamp(0.001, 1.0);
    }
}

impl AudioProcessor for Agc {
    fn process(&mut self, samples: &mut [f32]) {
        if !self.enabled {
            return;
        }

        for sample in samples.iter_mut() {
            let abs = sample.abs();

            // Gewuenschter Gain fuer diesen Sample
            let desired_gain = if abs > 1e-6 {
                (self.config.target_level / abs).clamp(self.config.min_gain, self.config.max_gain)
            } else {
                self.config.max_gain
            };

            // Gain glaetten: schnell runter (Attack), langsam hoch (Release)
            if desired_gain < self.current_gain {
                // Attack: Gain schnell reduzieren
                self.current_gain = self.config.attack_coeff * self.current_gain
                    + (1.0 - self.config.attack_coeff) * desired_gain;
            } else {
                // Release: Gain langsam erhoehen
                self.current_gain = self.config.release_coeff * self.current_gain
                    + (1.0 - self.config.release_coeff) * desired_gain;
            }

            let amplified = *sample * self.current_gain;

            // Hard Limiter
            *sample = amplified.clamp(
                -self.config.limiter_threshold,
                self.config.limiter_threshold,
            );
        }
    }

    fn reset(&mut self) {
        self.current_gain = 1.0;
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
    fn agc_verstaerkt_leises_signal() {
        let mut agc = Agc::new(AgcConfig {
            target_level: 0.5,
            max_gain: 50.0,
            min_gain: 0.1,
            attack_coeff: 0.0, // sofortige Reaktion
            release_coeff: 0.0,
            limiter_threshold: 0.99,
        });
        let mut samples = vec![0.01f32; 480];
        agc.process(&mut samples);
        // Signal sollte verstaerkt worden sein
        let avg: f32 = samples.iter().map(|s| s.abs()).sum::<f32>() / samples.len() as f32;
        assert!(
            avg > 0.01,
            "AGC sollte leises Signal verstaerken, avg={}",
            avg
        );
    }

    #[test]
    fn agc_limiter_verhindert_clipping() {
        let config = AgcConfig {
            target_level: 0.9,
            max_gain: 100.0,
            min_gain: 1.0,
            attack_coeff: 0.0,
            release_coeff: 0.0,
            limiter_threshold: 0.95,
        };
        let mut agc = Agc::new(config);
        let mut samples = vec![0.9f32; 480];
        agc.process(&mut samples);
        // Kein Sample darf den Limiter-Threshold ueberschreiten
        for s in &samples {
            assert!(s.abs() <= 0.96, "Limiter versagt: {}", s);
        }
    }

    #[test]
    fn agc_deaktiviert_unveraendert() {
        let mut agc = Agc::new(AgcConfig::default());
        agc.set_enabled(false);
        let original = vec![0.01f32; 480];
        let mut samples = original.clone();
        agc.process(&mut samples);
        assert_eq!(samples, original);
    }

    #[test]
    fn agc_reset_setzt_gain() {
        let mut agc = Agc::new(AgcConfig::default());
        agc.current_gain = 15.0;
        agc.reset();
        assert!((agc.current_gain() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn agc_max_gain_begrenzt() {
        let config = AgcConfig {
            target_level: 0.5,
            max_gain: 3.0,
            min_gain: 0.1,
            attack_coeff: 0.0,
            release_coeff: 0.0,
            limiter_threshold: 0.99,
        };
        let mut agc = Agc::new(config);
        // Bei sehr leisem Signal (0.001) wuerde der Gain normalerweise 500 sein
        let mut samples = vec![0.001f32; 960];
        agc.process(&mut samples);
        // Gain sollte auf max_gain=3.0 begrenzt sein
        assert!(
            agc.current_gain() <= 3.01,
            "Gain sollte begrenzt sein: {}",
            agc.current_gain()
        );
    }
}
