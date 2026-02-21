//! Audio-Geraete-Enumeration und -Auswahl
//!
//! Stellt Funktionen bereit um verfuegbare Audio-Geraete aufzulisten
//! und das gewuenschte Ein-/Ausgabegeraet auszuwaehlen.

use cpal::traits::{DeviceTrait, HostTrait};
use cpal::Device;
use tracing::{debug, warn};

use crate::error::{AudioError, AudioResult};

/// Repraesentiert ein Audio-Geraet mit seinen Eigenschaften
#[derive(Debug, Clone)]
pub struct AudioDevice {
    /// Interner Bezeichner (Index im cpal-Host)
    pub id: String,
    /// Anzeigename des Geraets
    pub name: String,
    /// Unterstuetzte Abtastraten
    pub sample_rates: Vec<u32>,
    /// Maximale Kanalanzahl
    pub channels: u16,
}

/// Listet alle verfuegbaren Eingabegeraete auf
pub fn list_input_devices() -> AudioResult<Vec<AudioDevice>> {
    let host = cpal::default_host();
    let devices = host
        .input_devices()
        .map_err(|e| AudioError::StreamFehler(e.to_string()))?;

    let mut result = Vec::new();
    for device in devices {
        match device_to_audio_device(&device) {
            Ok(ad) => result.push(ad),
            Err(e) => warn!("Eingabegeraet konnte nicht gelesen werden: {}", e),
        }
    }
    debug!("Gefundene Eingabegeraete: {}", result.len());
    Ok(result)
}

/// Listet alle verfuegbaren Ausgabegeraete auf
pub fn list_output_devices() -> AudioResult<Vec<AudioDevice>> {
    let host = cpal::default_host();
    let devices = host
        .output_devices()
        .map_err(|e| AudioError::StreamFehler(e.to_string()))?;

    let mut result = Vec::new();
    for device in devices {
        match device_to_audio_device(&device) {
            Ok(ad) => result.push(ad),
            Err(e) => warn!("Ausgabegeraet konnte nicht gelesen werden: {}", e),
        }
    }
    debug!("Gefundene Ausgabegeraete: {}", result.len());
    Ok(result)
}

/// Gibt das Standard-Eingabegeraet zurueck
pub fn get_default_input() -> Option<AudioDevice> {
    let host = cpal::default_host();
    host.default_input_device()
        .and_then(|d| device_to_audio_device(&d).ok())
}

/// Gibt das Standard-Ausgabegeraet zurueck
pub fn get_default_output() -> Option<AudioDevice> {
    let host = cpal::default_host();
    host.default_output_device()
        .and_then(|d| device_to_audio_device(&d).ok())
}

/// Sucht ein Eingabegeraet anhand seines Namens
pub fn find_input_device_by_name(name: &str) -> AudioResult<AudioDevice> {
    let devices = list_input_devices()?;
    devices
        .into_iter()
        .find(|d| d.name.contains(name))
        .ok_or_else(|| AudioError::GeraetNichtGefunden(name.to_string()))
}

/// Sucht ein Ausgabegeraet anhand seines Namens
pub fn find_output_device_by_name(name: &str) -> AudioResult<AudioDevice> {
    let devices = list_output_devices()?;
    devices
        .into_iter()
        .find(|d| d.name.contains(name))
        .ok_or_else(|| AudioError::GeraetNichtGefunden(name.to_string()))
}

/// Laedt ein cpal-Device anhand des Namens fuer Eingabe
pub fn load_cpal_input_device(name: Option<&str>) -> AudioResult<Device> {
    let host = cpal::default_host();
    match name {
        None => host
            .default_input_device()
            .ok_or(AudioError::KeinStandardEingabegeraet),
        Some(n) => {
            let devices = host
                .input_devices()
                .map_err(|e| AudioError::StreamFehler(e.to_string()))?;
            for device in devices {
                if let Ok(dev_name) = device.name() {
                    if dev_name.contains(n) {
                        return Ok(device);
                    }
                }
            }
            Err(AudioError::GeraetNichtGefunden(n.to_string()))
        }
    }
}

/// Laedt ein cpal-Device anhand des Namens fuer Ausgabe
pub fn load_cpal_output_device(name: Option<&str>) -> AudioResult<Device> {
    let host = cpal::default_host();
    match name {
        None => host
            .default_output_device()
            .ok_or(AudioError::KeinStandardAusgabegeraet),
        Some(n) => {
            let devices = host
                .output_devices()
                .map_err(|e| AudioError::StreamFehler(e.to_string()))?;
            for device in devices {
                if let Ok(dev_name) = device.name() {
                    if dev_name.contains(n) {
                        return Ok(device);
                    }
                }
            }
            Err(AudioError::GeraetNichtGefunden(n.to_string()))
        }
    }
}

// Hilfsfunktion: cpal Device -> AudioDevice
fn device_to_audio_device(device: &Device) -> AudioResult<AudioDevice> {
    let name = device
        .name()
        .map_err(|e| AudioError::StreamFehler(e.to_string()))?;

    let mut sample_rates = Vec::new();
    let mut max_channels = 1u16;

    if let Ok(configs) = device.supported_input_configs() {
        for cfg in configs {
            let min = cfg.min_sample_rate().0;
            let max = cfg.max_sample_rate().0;
            // Gaengige Raten pruefen
            for rate in [8000u32, 16000, 24000, 44100, 48000] {
                if rate >= min && rate <= max && !sample_rates.contains(&rate) {
                    sample_rates.push(rate);
                }
            }
            if cfg.channels() > max_channels {
                max_channels = cfg.channels();
            }
        }
    }
    if let Ok(configs) = device.supported_output_configs() {
        for cfg in configs {
            let min = cfg.min_sample_rate().0;
            let max = cfg.max_sample_rate().0;
            for rate in [8000u32, 16000, 24000, 44100, 48000] {
                if rate >= min && rate <= max && !sample_rates.contains(&rate) {
                    sample_rates.push(rate);
                }
            }
            if cfg.channels() > max_channels {
                max_channels = cfg.channels();
            }
        }
    }

    sample_rates.sort_unstable();

    Ok(AudioDevice {
        id: name.clone(),
        name,
        sample_rates,
        channels: max_channels,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "Benoetigt Audio-Hardware"]
    fn eingabegeraete_auflistbar() {
        let devices = list_input_devices().expect("Liste sollte abrufbar sein");
        println!(
            "Eingabegeraete: {:?}",
            devices.iter().map(|d| &d.name).collect::<Vec<_>>()
        );
    }

    #[test]
    #[ignore = "Benoetigt Audio-Hardware"]
    fn ausgabegeraete_auflistbar() {
        let devices = list_output_devices().expect("Liste sollte abrufbar sein");
        println!(
            "Ausgabegeraete: {:?}",
            devices.iter().map(|d| &d.name).collect::<Vec<_>>()
        );
    }

    #[test]
    fn audio_device_felder() {
        let dev = AudioDevice {
            id: "test-id".to_string(),
            name: "Test Mikrofon".to_string(),
            sample_rates: vec![16000, 48000],
            channels: 1,
        };
        assert_eq!(dev.id, "test-id");
        assert_eq!(dev.channels, 1);
        assert!(dev.sample_rates.contains(&48000));
    }
}
