//! Noise Gate mit konfigurierbarem Threshold
//!
//! Unterdrueckt Audiosignal unterhalb eines einstellbaren Schwellenwerts.
//! Nutzt Attack/Release-Zeiten und Hysterese um Pumpeffekte zu vermeiden.

use super::AudioProcessor;

/// Konfiguration fuer den Noise Gate
#[derive(Debug, Clone)]
pub struct NoiseGateConfig {
    /// Oeffnungs-Schwellenwert in dB (z.B. -40.0)
    pub threshold_open_db: f32,
    /// Schliess-Schwellenwert in dB (Hysterese, z.B. -45.0)
    pub threshold_close_db: f32,
    /// Attack-Zeit in Sekunden (wie schnell Gate oeffnet)
    pub attack_secs: f32,
    /// Release-Zeit in Sekunden (wie schnell Gate schliesst)
    pub release_secs: f32,
    /// Abtastrate in Hz
    pub sample_rate: f32,
}

impl Default for NoiseGateConfig {
    fn default() -> Self {
        Self {
            threshold_open_db: -40.0,
            threshold_close_db: -45.0,
            attack_secs: 0.005,
            release_secs: 0.1,
            sample_rate: 48000.0,
        }
    }
}

/// Zustand des Noise Gate
#[derive(Debug, Clone, Copy, PartialEq)]
enum GateState {
    Open,
    Closed,
}

/// Noise Gate Prozessor
pub struct NoiseGate {
    config: NoiseGateConfig,
    state: GateState,
    gain: f32,
    attack_coeff: f32,
    release_coeff: f32,
    threshold_open_linear: f32,
    threshold_close_linear: f32,
    enabled: bool,
}

impl NoiseGate {
    /// Erstellt einen neuen Noise Gate
    pub fn new(config: NoiseGateConfig) -> Self {
        let attack_coeff = Self::time_to_coeff(config.attack_secs, config.sample_rate);
        let release_coeff = Self::time_to_coeff(config.release_secs, config.sample_rate);
        let threshold_open_linear = db_to_linear(config.threshold_open_db);
        let threshold_close_linear = db_to_linear(config.threshold_close_db);

        Self {
            config,
            state: GateState::Closed,
            gain: 0.0,
            attack_coeff,
            release_coeff,
            threshold_open_linear,
            threshold_close_linear,
            enabled: true,
        }
    }

    /// Erstellt mit Standard-Konfiguration
    pub fn default_speech() -> Self {
        Self::new(NoiseGateConfig::default())
    }

    /// Setzt den Schwellenwert (dB) zur Laufzeit
    pub fn set_threshold(&mut self, open_db: f32, close_db: f32) {
        self.config.threshold_open_db = open_db;
        self.config.threshold_close_db = close_db;
        self.threshold_open_linear = db_to_linear(open_db);
        self.threshold_close_linear = db_to_linear(close_db);
    }

    /// Gibt den aktuellen Gain-Wert zurueck (0.0 = geschlossen, 1.0 = offen)
    pub fn current_gain(&self) -> f32 {
        self.gain
    }

    /// Gibt zurueck ob das Gate aktuell offen ist
    pub fn is_open(&self) -> bool {
        self.state == GateState::Open
    }

    fn time_to_coeff(time_secs: f32, sample_rate: f32) -> f32 {
        if time_secs <= 0.0 {
            return 0.0;
        }
        (-1.0 / (time_secs * sample_rate)).exp()
    }
}

impl AudioProcessor for NoiseGate {
    fn process(&mut self, samples: &mut [f32]) {
        if !self.enabled {
            return;
        }

        for sample in samples.iter_mut() {
            let level = sample.abs();

            // Hysterese-Logik: verschiedene Schwellenwerte fuer Oeffnen/Schliessen
            match self.state {
                GateState::Closed => {
                    if level >= self.threshold_open_linear {
                        self.state = GateState::Open;
                    }
                }
                GateState::Open => {
                    if level < self.threshold_close_linear {
                        self.state = GateState::Closed;
                    }
                }
            }

            // Gain mit Attack/Release glaetten
            let target = if self.state == GateState::Open {
                1.0f32
            } else {
                0.0f32
            };

            if target > self.gain {
                // Attack: schnell oeffnen
                self.gain = self.attack_coeff * self.gain + (1.0 - self.attack_coeff) * target;
            } else {
                // Release: langsam schliessen
                self.gain = self.release_coeff * self.gain + (1.0 - self.release_coeff) * target;
            }

            *sample *= self.gain;
        }
    }

    fn reset(&mut self) {
        self.state = GateState::Closed;
        self.gain = 0.0;
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
}

fn db_to_linear(db: f32) -> f32 {
    10.0f32.powf(db / 20.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_gate_stilles_signal_gedaempft() {
        let mut gate = NoiseGate::default_speech();
        // Sehr leises Signal weit unter Threshold
        let mut samples = vec![0.0001f32; 480];
        gate.process(&mut samples);
        // Gate sollte geschlossen sein - Signalpegel sehr gering
        let energy: f32 = samples.iter().map(|s| s * s).sum();
        assert!(energy < 0.001, "Stilles Signal sollte gedaempft werden");
    }

    #[test]
    fn noise_gate_lautes_signal_passiert() {
        let config = NoiseGateConfig {
            threshold_open_db: -40.0,
            threshold_close_db: -45.0,
            attack_secs: 0.0, // sofortige Attack fuer Test
            release_secs: 0.1,
            sample_rate: 48000.0,
        };
        let mut gate = NoiseGate::new(config);
        // Starkes Signal bei 0.5 (entspricht ca. -6dB, weit ueber -40dB)
        let mut samples = vec![0.5f32; 960];
        gate.process(&mut samples);
        // Die letzten Samples sollten voll durchkommen (Gate offen)
        let last = samples[samples.len() - 1];
        assert!(
            last > 0.4,
            "Lautes Signal sollte Gate oeffnen, last={}",
            last
        );
    }

    #[test]
    fn noise_gate_hysterese_open_threshold_hoeher() {
        let config = NoiseGateConfig {
            threshold_open_db: -30.0,
            threshold_close_db: -40.0,
            attack_secs: 0.001,
            release_secs: 0.01,
            sample_rate: 48000.0,
        };
        let gate = NoiseGate::new(config);
        assert!(gate.threshold_open_linear > gate.threshold_close_linear);
    }

    #[test]
    fn noise_gate_reset() {
        let mut gate = NoiseGate::default_speech();
        let mut samples = vec![0.5f32; 960];
        gate.process(&mut samples);
        gate.reset();
        assert_eq!(gate.gain, 0.0);
        assert!(!gate.is_open());
    }

    #[test]
    fn noise_gate_deaktiviert_passiert_alles() {
        let mut gate = NoiseGate::default_speech();
        gate.set_enabled(false);
        let original = vec![0.001f32; 480];
        let mut samples = original.clone();
        gate.process(&mut samples);
        // Deaktiviert = keine Veraenderung
        for (orig, processed) in original.iter().zip(samples.iter()) {
            assert_eq!(orig, processed);
        }
    }

    #[test]
    fn noise_gate_threshold_aenderbar() {
        let mut gate = NoiseGate::default_speech();
        gate.set_threshold(-20.0, -25.0);
        assert!((gate.config.threshold_open_db - (-20.0)).abs() < f32::EPSILON);
    }

    #[test]
    fn db_to_linear_korrekt() {
        // 0 dB = 1.0
        assert!((db_to_linear(0.0) - 1.0).abs() < 0.001);
        // -20 dB ≈ 0.1
        assert!((db_to_linear(-20.0) - 0.1).abs() < 0.001);
        // -40 dB ≈ 0.01
        assert!((db_to_linear(-40.0) - 0.01).abs() < 0.001);
    }
}
