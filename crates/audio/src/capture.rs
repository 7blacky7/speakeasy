//! Mikrofon-Capture via cpal
//!
//! Oeffnet einen cpal InputStream und schreibt Samples in einen
//! lock-free Ring-Buffer. Die Verarbeitung laeuft im cpal-Callback.

use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{Device, SampleFormat, Stream, StreamConfig};
use ringbuf::traits::{Producer, Split};
use ringbuf::{HeapCons, HeapProd, HeapRb};
use tracing::{debug, error, warn};

use crate::error::{AudioError, AudioResult};

/// Konfiguration fuer den Audio-Capture
#[derive(Debug, Clone)]
pub struct CaptureConfig {
    /// Abtastrate in Hz
    pub sample_rate: u32,
    /// Kanalanzahl (1 = Mono, 2 = Stereo)
    pub channels: u16,
    /// Ring-Buffer Kapazitaet in Samples
    pub buffer_size: usize,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48000,
            channels: 1,
            buffer_size: 48000 * 2, // 2 Sekunden Puffer
        }
    }
}

/// Produziert Samples aus dem Mikrofon-Callback
pub type CaptureProducer = HeapProd<f32>;
/// Konsumiert Samples fuer die Verarbeitung
pub type CaptureConsumer = HeapCons<f32>;

/// Audio-Capture-Stream
///
/// Haelt den cpal-Stream am Leben. Wird der CaptureStream gedroppt,
/// stoppt die Aufnahme automatisch.
pub struct CaptureStream {
    _stream: Stream,
    config: CaptureConfig,
}

impl CaptureStream {
    /// Gibt die Konfiguration des Streams zurueck
    pub fn config(&self) -> &CaptureConfig {
        &self.config
    }
}

/// Oeffnet einen Capture-Stream auf dem gegebenen Geraet.
///
/// Gibt den Stream und den Ring-Buffer Consumer zurueck.
/// Der Producer laeuft im cpal-Callback-Thread.
pub fn open_capture_stream(
    device: &Device,
    config: CaptureConfig,
) -> AudioResult<(CaptureStream, CaptureConsumer)> {
    let stream_config = StreamConfig {
        channels: config.channels,
        sample_rate: cpal::SampleRate(config.sample_rate),
        buffer_size: cpal::BufferSize::Default,
    };

    let rb = HeapRb::<f32>::new(config.buffer_size);
    let (mut producer, consumer) = rb.split();

    // cpal-Callback: schreibt Samples in den Ring-Buffer
    let err_fn = |err| error!("Capture-Fehler: {}", err);

    // Unterstuetzte Sample-Formate pruefen
    let supported = device
        .supported_input_configs()
        .map_err(|e| AudioError::StreamFehler(e.to_string()))?
        .find(|c| {
            c.min_sample_rate().0 <= config.sample_rate
                && c.max_sample_rate().0 >= config.sample_rate
                && c.channels() >= config.channels
        });

    let sample_format = supported
        .map(|c| c.sample_format())
        .unwrap_or(SampleFormat::F32);

    let stream = match sample_format {
        SampleFormat::F32 => device
            .build_input_stream(
                &stream_config,
                move |data: &[f32], _| {
                    let written = producer.push_slice(data);
                    if written < data.len() {
                        warn!(
                            "Capture Ring-Buffer voll, {} Samples verworfen",
                            data.len() - written
                        );
                    }
                },
                err_fn,
                None,
            )
            .map_err(|e| AudioError::StreamFehler(e.to_string()))?,
        SampleFormat::I16 => device
            .build_input_stream(
                &stream_config,
                move |data: &[i16], _| {
                    let floats: Vec<f32> =
                        data.iter().map(|&s| s as f32 / i16::MAX as f32).collect();
                    let written = producer.push_slice(&floats);
                    if written < floats.len() {
                        warn!("Capture Ring-Buffer voll");
                    }
                },
                err_fn,
                None,
            )
            .map_err(|e| AudioError::StreamFehler(e.to_string()))?,
        SampleFormat::U8 => device
            .build_input_stream(
                &stream_config,
                move |data: &[u8], _| {
                    let floats: Vec<f32> =
                        data.iter().map(|&s| (s as f32 - 128.0) / 128.0).collect();
                    let written = producer.push_slice(&floats);
                    if written < floats.len() {
                        warn!("Capture Ring-Buffer voll");
                    }
                },
                err_fn,
                None,
            )
            .map_err(|e| AudioError::StreamFehler(e.to_string()))?,
        _ => {
            return Err(AudioError::StreamFehler(format!(
                "Nicht unterstuetztes Sample-Format: {:?}",
                sample_format
            )))
        }
    };

    stream
        .play()
        .map_err(|e| AudioError::StreamFehler(e.to_string()))?;

    debug!(
        "Capture-Stream geoeffnet: {}Hz {}ch",
        config.sample_rate, config.channels
    );

    Ok((
        CaptureStream {
            _stream: stream,
            config,
        },
        consumer,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cpal::traits::HostTrait;

    #[test]
    fn capture_config_default() {
        let config = CaptureConfig::default();
        assert_eq!(config.sample_rate, 48000);
        assert_eq!(config.channels, 1);
        assert!(config.buffer_size > 0);
    }

    #[test]
    #[ignore = "Benoetigt Audio-Hardware"]
    fn capture_stream_oeffnen() {
        let host = cpal::default_host();
        if let Some(device) = host.default_input_device() {
            let config = CaptureConfig::default();
            let result = open_capture_stream(&device, config);
            assert!(result.is_ok(), "Capture-Stream sollte oeffenbar sein");
        }
    }
}
