//! Audio-Processing-Pipeline
//!
//! Verbindet DSP-Module in konfigurierbarer Reihenfolge.
//! Trennung zwischen Capture-Pipeline (Mikrofon -> Encode) und
//! Playback-Pipeline (Decode -> Volume -> Output).

use crate::dsp::AudioProcessor;

/// Ergebnis eines verarbeiteten Frames
#[derive(Debug, Clone)]
pub struct ProcessedFrame {
    /// Verarbeitete Samples
    pub samples: Vec<f32>,
    /// Ob Sprache erkannt wurde (nach VAD/PTT)
    pub voice_active: bool,
    /// Anzahl der angewendeten Prozessoren
    pub processors_applied: usize,
}

/// Audio-Verarbeitungs-Pipeline
///
/// Wendet eine Kette von `AudioProcessor`-Implementierungen
/// sequenziell auf jeden Frame an.
pub struct AudioPipeline {
    processors: Vec<Box<dyn AudioProcessor>>,
    voice_active: bool,
}

impl AudioPipeline {
    /// Erstellt eine neue Pipeline mit der gegebenen Prozessor-Kette
    pub fn new(processors: Vec<Box<dyn AudioProcessor>>) -> Self {
        Self {
            processors,
            voice_active: false,
        }
    }

    /// Leere Pipeline ohne Prozessoren
    pub fn empty() -> Self {
        Self::new(Vec::new())
    }

    /// Verarbeitet einen Frame durch die gesamte Pipeline
    pub fn process_frame(&mut self, input: &[f32]) -> ProcessedFrame {
        let mut samples = input.to_vec();
        let processors_applied = self.processors.iter().filter(|p| p.is_enabled()).count();

        for processor in self.processors.iter_mut() {
            processor.process(&mut samples);
        }

        ProcessedFrame {
            samples,
            voice_active: self.voice_active,
            processors_applied,
        }
    }

    /// Setzt den Sprache-Aktiv-Status (von VAD oder PTT gesetzt)
    pub fn set_voice_active(&mut self, active: bool) {
        self.voice_active = active;
    }

    /// Gibt zurueck ob aktuell Sprache aktiv ist
    pub fn is_voice_active(&self) -> bool {
        self.voice_active
    }

    /// Fuegt einen Prozessor am Ende der Pipeline ein
    pub fn push(&mut self, processor: Box<dyn AudioProcessor>) {
        self.processors.push(processor);
    }

    /// Gibt die Anzahl der Prozessoren zurueck
    pub fn len(&self) -> usize {
        self.processors.len()
    }

    /// Gibt zurueck ob die Pipeline leer ist
    pub fn is_empty(&self) -> bool {
        self.processors.is_empty()
    }

    /// Setzt alle Prozessoren zurueck
    pub fn reset_all(&mut self) {
        for p in self.processors.iter_mut() {
            p.reset();
        }
    }

    /// Aktiviert oder deaktiviert alle Prozessoren
    pub fn set_all_enabled(&mut self, enabled: bool) {
        for p in self.processors.iter_mut() {
            p.set_enabled(enabled);
        }
    }
}

/// Erstellt die Standard-Capture-Pipeline
///
/// Reihenfolge: NoiseGate -> NoiseSuppression -> AGC -> EchoCancellation -> DeEsser
pub fn build_default_capture_pipeline() -> AudioPipeline {
    use crate::dsp::{
        agc::{Agc, AgcConfig},
        deesser::{DeEsser, DeEsserConfig},
        echo_cancel::{EchoCancelConfig, EchoCanceller},
        noise_gate::{NoiseGate, NoiseGateConfig},
        noise_suppression::{NoiseSuppressor, SuppressionLevel},
    };

    AudioPipeline::new(vec![
        Box::new(NoiseGate::new(NoiseGateConfig::default())),
        Box::new(NoiseSuppressor::new(SuppressionLevel::Medium)),
        Box::new(Agc::new(AgcConfig::default())),
        Box::new(EchoCanceller::new(EchoCancelConfig::default())),
        Box::new(DeEsser::new(DeEsserConfig::default())),
    ])
}

/// Erstellt eine minimale Pipeline (nur Noise Gate + AGC)
pub fn build_minimal_capture_pipeline() -> AudioPipeline {
    use crate::dsp::{
        agc::{Agc, AgcConfig},
        noise_gate::{NoiseGate, NoiseGateConfig},
    };

    AudioPipeline::new(vec![
        Box::new(NoiseGate::new(NoiseGateConfig::default())),
        Box::new(Agc::new(AgcConfig::default())),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::{
        agc::{Agc, AgcConfig},
        noise_gate::{NoiseGate, NoiseGateConfig},
    };

    #[test]
    fn pipeline_leer_passiert_unveraendert() {
        let mut pipeline = AudioPipeline::empty();
        let input = vec![0.5f32; 480];
        let result = pipeline.process_frame(&input);
        assert_eq!(result.samples, input);
        assert_eq!(result.processors_applied, 0);
    }

    #[test]
    fn pipeline_mit_prozessoren() {
        let mut pipeline = AudioPipeline::new(vec![
            Box::new(NoiseGate::new(NoiseGateConfig::default())),
            Box::new(Agc::new(AgcConfig::default())),
        ]);
        assert_eq!(pipeline.len(), 2);
        let result = pipeline.process_frame(&vec![0.5f32; 480]);
        assert_eq!(result.samples.len(), 480);
    }

    #[test]
    fn pipeline_push() {
        let mut pipeline = AudioPipeline::empty();
        assert!(pipeline.is_empty());
        pipeline.push(Box::new(NoiseGate::new(NoiseGateConfig::default())));
        assert_eq!(pipeline.len(), 1);
    }

    #[test]
    fn pipeline_reset_all() {
        let mut pipeline = build_default_capture_pipeline();
        pipeline.reset_all(); // Darf nicht panic
    }

    #[test]
    fn pipeline_set_all_disabled() {
        let mut pipeline = build_default_capture_pipeline();
        pipeline.set_all_enabled(false);
        let input = vec![0.001f32; 480];
        let result = pipeline.process_frame(&input);
        // Alle deaktiviert -> unveraendert
        assert_eq!(result.samples, input);
        assert_eq!(result.processors_applied, 0);
    }

    #[test]
    fn pipeline_voice_active_flag() {
        let mut pipeline = AudioPipeline::empty();
        assert!(!pipeline.is_voice_active());
        pipeline.set_voice_active(true);
        let result = pipeline.process_frame(&[0.5f32; 4]);
        assert!(result.voice_active);
    }

    #[test]
    fn default_capture_pipeline_hat_5_prozessoren() {
        let pipeline = build_default_capture_pipeline();
        assert_eq!(pipeline.len(), 5);
    }

    #[test]
    fn minimal_pipeline_hat_2_prozessoren() {
        let pipeline = build_minimal_capture_pipeline();
        assert_eq!(pipeline.len(), 2);
    }

    #[test]
    fn pipeline_verarbeitet_frame_laenge_erhalten() {
        let mut pipeline = build_default_capture_pipeline();
        let input = vec![0.1f32; 960];
        let result = pipeline.process_frame(&input);
        assert_eq!(
            result.samples.len(),
            960,
            "Frame-Laenge muss erhalten bleiben"
        );
    }
}
