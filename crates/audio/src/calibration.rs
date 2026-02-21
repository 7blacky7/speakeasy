//! Audio-Kalibrierung und Auto-Setup
//!
//! Misst den Umgebungsgeraeuschpegel und schlaegt optimale
//! Schwellenwerte fuer Noise Gate und VAD vor.

use crate::dsp::vad::rms_energy;
use crate::error::{AudioError, AudioResult};

/// Ergebnis einer Kalibrierungsmessung
#[derive(Debug, Clone)]
pub struct CalibrationResult {
    /// Gemessener Grundrauschpegel in dBFS
    pub noise_floor_db: f32,
    /// Empfohlener Noise Gate Threshold (open) in dBFS
    pub suggested_gate_threshold_db: f32,
    /// Empfohlener Noise Gate Threshold (close) mit Hysterese
    pub suggested_gate_close_db: f32,
    /// Empfohlener VAD Energie-Schwellenwert (normalisiert)
    pub suggested_vad_threshold: f32,
    /// Anzahl der gemessenen Frames
    pub frames_measured: u32,
    /// Peak-Pegel waehrend der Messung in dBFS
    pub peak_db: f32,
}

/// Kalibriert den Noise Floor aus PCM-Samples
///
/// `samples` sollte Stille oder Umgebungsgeraeusch enthalten.
/// `sample_rate` und `frame_size` definieren wie die Frames aufgeteilt werden.
pub fn calibrate_from_samples(
    samples: &[f32],
    frame_size: usize,
) -> AudioResult<CalibrationResult> {
    if samples.is_empty() || frame_size == 0 {
        return Err(AudioError::Konfiguration(
            "Samples und Frame-Groesse muessen > 0 sein".to_string(),
        ));
    }

    if samples.len() < frame_size {
        return Err(AudioError::Konfiguration(format!(
            "Zu wenige Samples fuer Kalibrierung: {} < {}",
            samples.len(),
            frame_size
        )));
    }

    let mut energies: Vec<f32> = Vec::new();
    let mut peak: f32 = 0.0;

    for chunk in samples.chunks(frame_size) {
        if chunk.len() < frame_size {
            break;
        }
        let energy = rms_energy(chunk);
        energies.push(energy);

        let frame_peak = chunk.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        if frame_peak > peak {
            peak = frame_peak;
        }
    }

    if energies.is_empty() {
        return Err(AudioError::KalibrierungsTimeout);
    }

    // Median-aehnliche Schaetzung: Durchschnitt der unteren 75% (ignoriert Spitzen)
    energies.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let cutoff = (energies.len() * 3 / 4).max(1);
    let noise_floor_rms: f32 = energies[..cutoff].iter().sum::<f32>() / cutoff as f32;

    let noise_floor_db = linear_to_db(noise_floor_rms);
    let peak_db = linear_to_db(peak);

    // Noise Gate: 6 dB ueber Noise Floor oeffnen, 11 dB ueber Floor schliessen
    let suggested_gate_threshold_db = noise_floor_db + 6.0;
    let suggested_gate_close_db = noise_floor_db + 1.0;

    // VAD: Energie etwas ueber Noise Floor (linear)
    let suggested_vad_threshold = noise_floor_rms * 3.0;

    Ok(CalibrationResult {
        noise_floor_db,
        suggested_gate_threshold_db,
        suggested_gate_close_db,
        suggested_vad_threshold,
        frames_measured: energies.len() as u32,
        peak_db,
    })
}

/// Synthetische Kalibrierung ohne Hardware
///
/// Nuetzlich wenn kein Mikrofon verfuegbar ist (Fallback / Tests).
pub fn default_calibration() -> CalibrationResult {
    CalibrationResult {
        noise_floor_db: -60.0,
        suggested_gate_threshold_db: -54.0,
        suggested_gate_close_db: -59.0,
        suggested_vad_threshold: 0.001,
        frames_measured: 0,
        peak_db: -60.0,
    }
}

fn linear_to_db(linear: f32) -> f32 {
    if linear <= 0.0 {
        return -100.0;
    }
    20.0 * linear.log10()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kalibrierung_stille() {
        let samples = vec![0.001f32; 48000]; // 1 Sekunde Stille
        let result = calibrate_from_samples(&samples, 480).unwrap();
        assert!(result.noise_floor_db < -40.0, "Stille sollte niedrigen Pegel haben");
        assert!(result.suggested_gate_threshold_db > result.noise_floor_db);
        assert!(result.frames_measured > 0);
    }

    #[test]
    fn kalibrierung_empfehlungen_logisch() {
        let samples = vec![0.01f32; 48000];
        let result = calibrate_from_samples(&samples, 480).unwrap();
        // Gate-Threshold sollte ueber Noise Floor liegen
        assert!(result.suggested_gate_threshold_db > result.noise_floor_db);
        // Close-Threshold sollte unter Open-Threshold liegen (Hysterese)
        assert!(result.suggested_gate_close_db < result.suggested_gate_threshold_db);
        // VAD-Threshold sollte positiv sein
        assert!(result.suggested_vad_threshold > 0.0);
    }

    #[test]
    fn kalibrierung_lautes_signal() {
        let samples = vec![0.5f32; 48000];
        let result = calibrate_from_samples(&samples, 480).unwrap();
        assert!(result.noise_floor_db > -20.0, "Lautes Signal sollte hohen Pegel haben");
    }

    #[test]
    fn kalibrierung_zu_wenige_samples() {
        let samples = vec![0.1f32; 100];
        let result = calibrate_from_samples(&samples, 480);
        assert!(result.is_err());
    }

    #[test]
    fn kalibrierung_leere_samples() {
        let result = calibrate_from_samples(&[], 480);
        assert!(result.is_err());
    }

    #[test]
    fn kalibrierung_zero_frame_size() {
        let result = calibrate_from_samples(&[0.1f32; 1000], 0);
        assert!(result.is_err());
    }

    #[test]
    fn default_kalibrierung_sinnvoll() {
        let cal = default_calibration();
        assert!(cal.noise_floor_db < 0.0);
        assert!(cal.suggested_gate_threshold_db > cal.noise_floor_db);
        assert!(cal.suggested_vad_threshold > 0.0);
    }

    #[test]
    fn linear_to_db_korrekt() {
        assert!((linear_to_db(1.0) - 0.0).abs() < 0.01);
        assert!((linear_to_db(0.1) - (-20.0)).abs() < 0.01);
    }

    #[test]
    fn kalibrierung_frames_gezaehlt() {
        // 4800 Samples / 480 frame_size = 10 Frames
        let samples = vec![0.01f32; 4800];
        let result = calibrate_from_samples(&samples, 480).unwrap();
        assert_eq!(result.frames_measured, 10);
    }
}
