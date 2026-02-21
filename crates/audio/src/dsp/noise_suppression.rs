//! Rauschunterdrueckung via spektrale Subtraktion
//!
//! Drei Stufen: Niedrig, Mittel, Hoch.
//! Schaetzt das Rauschspektrum waehrend Stille und subtrahiert es.

use super::AudioProcessor;

/// Stufe der Rauschunterdrueckung
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SuppressionLevel {
    /// Leichte Rauschreduzierung (alpha = 1.5)
    Low,
    /// Mittlere Rauschreduzierung (alpha = 2.5)
    #[default]
    Medium,
    /// Starke Rauschreduzierung (alpha = 4.0)
    High,
}

impl SuppressionLevel {
    /// Subtraktions-Faktor (alpha)
    fn alpha(&self) -> f32 {
        match self {
            Self::Low => 1.5,
            Self::Medium => 2.5,
            Self::High => 4.0,
        }
    }

    /// Minimaler Gain nach Subtraktion (Floor, verhindert musical noise)
    fn spectral_floor(&self) -> f32 {
        match self {
            Self::Low => 0.2,
            Self::Medium => 0.1,
            Self::High => 0.05,
        }
    }
}

/// Vereinfachter Rauschunterdruecker (spektrale Subtraktion im Zeitbereich)
///
/// Da echte FFT-basierte spektrale Subtraktion viel Komplexitaet bedingt,
/// wird hier eine Band-Energie-basierte Naeherung verwendet: Der geschaetzte
/// Rauschpegel wird per exponentieller Glaettung aktualisiert und vom
/// Signal subtrahiert.
pub struct NoiseSuppressor {
    level: SuppressionLevel,
    /// Geschaetzter Rauschpegel (RMS)
    noise_estimate: f32,
    /// Glaettungsfaktor fuer Rauschschaetzung
    noise_smoothing: f32,
    /// Ob gerade Stille vorliegt (fuer Rauschschaetzung)
    in_noise: bool,
    /// Stille-Detektor: Frames unterhalb dieses Pegels gelten als Rauschen
    silence_threshold: f32,
    enabled: bool,
}

impl NoiseSuppressor {
    pub fn new(level: SuppressionLevel) -> Self {
        Self {
            level,
            noise_estimate: 0.0,
            noise_smoothing: 0.95,
            in_noise: true,
            silence_threshold: 0.02,
            enabled: true,
        }
    }

    /// Setzt den Stille-Schwellenwert (RMS-Wert unterhalb dessen Rauschen geschaetzt wird)
    pub fn set_silence_threshold(&mut self, threshold: f32) {
        self.silence_threshold = threshold;
    }

    /// Gibt die aktuelle Rauschschaetzung zurueck
    pub fn noise_estimate(&self) -> f32 {
        self.noise_estimate
    }

    /// Setzt die Unterdrueckungsstufe
    pub fn set_level(&mut self, level: SuppressionLevel) {
        self.level = level;
    }

    fn rms(samples: &[f32]) -> f32 {
        if samples.is_empty() {
            return 0.0;
        }
        let sum_sq: f32 = samples.iter().map(|s| s * s).sum();
        (sum_sq / samples.len() as f32).sqrt()
    }
}

impl AudioProcessor for NoiseSuppressor {
    fn process(&mut self, samples: &mut [f32]) {
        if !self.enabled {
            return;
        }

        let frame_rms = Self::rms(samples);

        // Rauschschaetzung: nur aktualisieren wenn Signal leise (= Rauschen)
        if frame_rms < self.silence_threshold {
            self.in_noise = true;
            // Rauschpegel nach oben oder unten glaetten
            self.noise_estimate = self.noise_smoothing * self.noise_estimate
                + (1.0 - self.noise_smoothing) * frame_rms;
        } else {
            self.in_noise = false;
        }

        if self.noise_estimate < 1e-7 {
            return;
        }

        let alpha = self.level.alpha();
        let floor = self.level.spectral_floor();

        // Spektrale Subtraktion (Zeitbereich-Naeherung):
        // Gain = max(floor, 1 - alpha * (noise / signal))
        let gain = if frame_rms > 1e-7 {
            let ratio = self.noise_estimate / frame_rms;
            (1.0 - alpha * ratio).max(floor)
        } else {
            floor
        };

        for sample in samples.iter_mut() {
            *sample *= gain;
        }
    }

    fn reset(&mut self) {
        self.noise_estimate = 0.0;
        self.in_noise = true;
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
    fn suppressor_daempft_rauschen() {
        let mut ns = NoiseSuppressor::new(SuppressionLevel::High);
        // Erstmal Rauschpegel lernen lassen
        for _ in 0..20 {
            let mut frame = vec![0.005f32; 480];
            ns.process(&mut frame);
        }
        // Jetzt Rauschen verarbeiten - sollte gedaempft werden
        let mut rauschen = vec![0.005f32; 480];
        ns.process(&mut rauschen);
        let rms_nach: f32 = rauschen.iter().map(|s| s * s).sum::<f32>() / 480.0;
        assert!(
            rms_nach < 0.005f32 * 0.005f32,
            "Rauschen sollte reduziert sein, RMS^2={}",
            rms_nach
        );
    }

    #[test]
    fn suppressor_stufen_alpha() {
        assert!(SuppressionLevel::High.alpha() > SuppressionLevel::Medium.alpha());
        assert!(SuppressionLevel::Medium.alpha() > SuppressionLevel::Low.alpha());
    }

    #[test]
    fn suppressor_stufen_floor() {
        assert!(SuppressionLevel::High.spectral_floor() < SuppressionLevel::Low.spectral_floor());
    }

    #[test]
    fn suppressor_deaktiviert_unveraendert() {
        let mut ns = NoiseSuppressor::new(SuppressionLevel::Medium);
        ns.set_enabled(false);
        let original = vec![0.01f32; 480];
        let mut samples = original.clone();
        ns.process(&mut samples);
        assert_eq!(samples, original);
    }

    #[test]
    fn suppressor_reset() {
        let mut ns = NoiseSuppressor::new(SuppressionLevel::Low);
        let mut frame = vec![0.01f32; 480];
        ns.process(&mut frame);
        ns.reset();
        assert_eq!(ns.noise_estimate(), 0.0);
    }

    #[test]
    fn suppressor_level_aenderbar() {
        let mut ns = NoiseSuppressor::new(SuppressionLevel::Low);
        ns.set_level(SuppressionLevel::High);
        assert_eq!(ns.level, SuppressionLevel::High);
    }
}
