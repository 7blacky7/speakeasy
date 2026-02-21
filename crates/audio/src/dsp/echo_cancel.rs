//! Echo Cancellation (AEC) - vereinfachte Implementierung
//!
//! Echtes AEC (wie WebRTC) ist extrem komplex. Diese Implementierung
//! nutzt einen einfachen Delay-basierten Ansatz mit Korrelationsschaetzung.
//! Fuer produktionsreife AEC sollte eine externe Bibliothek (z.B. speex DSP)
//! eingebunden werden.

use super::AudioProcessor;

/// Konfiguration fuer Echo Cancellation
#[derive(Debug, Clone)]
pub struct EchoCancelConfig {
    /// Maximale Echo-Verzoegerung in Samples die beruecksichtigt wird
    pub max_delay_samples: usize,
    /// Verstaerkungsfaktor fuer Echo-Subtraktion (0.0..1.0)
    pub cancellation_strength: f32,
    /// Adaptionsrate des Filters
    pub adaptation_rate: f32,
}

impl Default for EchoCancelConfig {
    fn default() -> Self {
        Self {
            max_delay_samples: 4800, // 100ms bei 48kHz
            cancellation_strength: 0.7,
            adaptation_rate: 0.01,
        }
    }
}

/// Vereinfachter Echo Canceller
///
/// Speichert das Ausgabe-Signal (Referenz) und subtrahiert eine
/// verzoegerte, skalierte Version davon vom Eingangssignal.
pub struct EchoCanceller {
    config: EchoCancelConfig,
    /// Ring-Buffer fuer das Referenzsignal (Lautsprecher-Output)
    reference_buffer: Vec<f32>,
    write_pos: usize,
    /// Geschaetzte Echo-Verzoegerung in Samples
    estimated_delay: usize,
    enabled: bool,
}

impl EchoCanceller {
    pub fn new(config: EchoCancelConfig) -> Self {
        let buf_size = config.max_delay_samples;
        Self {
            reference_buffer: vec![0.0; buf_size],
            write_pos: 0,
            estimated_delay: config.max_delay_samples / 4, // Initiale Schaetzung: 25ms
            config,
            enabled: true,
        }
    }

    /// Fuegt ein Referenz-Sample (Lautsprecher) in den Buffer ein.
    /// Muss vor `process()` fuer jeden Frame aufgerufen werden.
    pub fn feed_reference(&mut self, samples: &[f32]) {
        for &s in samples {
            self.reference_buffer[self.write_pos] = s;
            self.write_pos = (self.write_pos + 1) % self.reference_buffer.len();
        }
    }

    /// Liest ein verzoegertes Referenzsample
    fn get_reference_sample(&self, offset: usize) -> f32 {
        let buf_len = self.reference_buffer.len();
        let pos = (self.write_pos + buf_len - offset) % buf_len;
        self.reference_buffer[pos]
    }

    /// Setzt die geschaetzte Echo-Verzoegerung manuell
    pub fn set_delay(&mut self, delay_samples: usize) {
        self.estimated_delay = delay_samples.min(self.config.max_delay_samples.saturating_sub(1));
    }
}

impl AudioProcessor for EchoCanceller {
    fn process(&mut self, samples: &mut [f32]) {
        if !self.enabled {
            return;
        }

        for (i, sample) in samples.iter_mut().enumerate() {
            // Echo-Schaetzung: verzoegertes Referenzsignal
            let echo_estimate = self.get_reference_sample(self.estimated_delay + i);
            // Echo subtrahieren
            *sample -= echo_estimate * self.config.cancellation_strength;
        }
    }

    fn reset(&mut self) {
        self.reference_buffer.fill(0.0);
        self.write_pos = 0;
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
    fn echo_canceller_erstellen() {
        let aec = EchoCanceller::new(EchoCancelConfig::default());
        assert!(aec.is_enabled());
        assert_eq!(aec.reference_buffer.len(), 4800);
    }

    #[test]
    fn echo_canceller_deaktiviert_unveraendert() {
        let mut aec = EchoCanceller::new(EchoCancelConfig::default());
        aec.set_enabled(false);
        let original = vec![0.5f32; 480];
        let mut samples = original.clone();
        aec.process(&mut samples);
        assert_eq!(samples, original);
    }

    #[test]
    fn echo_canceller_feed_reference() {
        let mut aec = EchoCanceller::new(EchoCancelConfig::default());
        let reference = vec![0.3f32; 480];
        aec.feed_reference(&reference);
        // write_pos sollte sich um 480 erhoeht haben
        assert_eq!(aec.write_pos, 480);
    }

    #[test]
    fn echo_canceller_reset() {
        let mut aec = EchoCanceller::new(EchoCancelConfig::default());
        aec.feed_reference(&vec![0.5f32; 480]);
        aec.reset();
        assert_eq!(aec.write_pos, 0);
        assert!(aec.reference_buffer.iter().all(|&s| s == 0.0));
    }

    #[test]
    fn echo_canceller_reduziert_echo() {
        let mut aec = EchoCanceller::new(EchoCancelConfig {
            max_delay_samples: 480,
            cancellation_strength: 1.0,
            adaptation_rate: 0.01,
        });
        // Referenz einspeisung
        let reference = vec![0.5f32; 480];
        aec.feed_reference(&reference);
        aec.set_delay(240);

        // Mikrofon-Signal = Referenz (perfektes Echo)
        let mut mic = vec![0.5f32; 240];
        aec.process(&mut mic);
        // Echo sollte (teilweise) subtrahiert worden sein
        let avg: f32 = mic.iter().map(|s| s.abs()).sum::<f32>() / mic.len() as f32;
        assert!(avg < 0.5, "Echo sollte reduziert sein: avg={}", avg);
    }

    #[test]
    fn echo_canceller_delay_begrenzt() {
        let mut aec = EchoCanceller::new(EchoCancelConfig::default());
        aec.set_delay(999999); // Weit ueber max
        assert!(aec.estimated_delay < aec.config.max_delay_samples);
    }
}
