//! Audio-Playback via cpal
//!
//! Oeffnet einen cpal OutputStream und liest Samples aus einem
//! lock-free Ring-Buffer. Unterstuetzt Mixing mehrerer Quellen.

use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{Device, SampleFormat, Stream, StreamConfig};
use ringbuf::{HeapRb, HeapProd, HeapCons};
use ringbuf::traits::{Split, Consumer};
use tracing::{debug, error, warn};

use crate::error::{AudioError, AudioResult};

/// Konfiguration fuer den Audio-Playback
#[derive(Debug, Clone)]
pub struct PlaybackConfig {
    /// Abtastrate in Hz
    pub sample_rate: u32,
    /// Kanalanzahl
    pub channels: u16,
    /// Ring-Buffer Kapazitaet in Samples
    pub buffer_size: usize,
}

impl Default for PlaybackConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48000,
            channels: 1,
            buffer_size: 48000 * 2,
        }
    }
}

/// Produziert Samples fuer den Playback-Callback
pub type PlaybackProducer = HeapProd<f32>;
/// Konsumiert Samples im cpal-Callback
pub type PlaybackConsumer = HeapCons<f32>;

/// Audio-Playback-Stream
pub struct PlaybackStream {
    _stream: Stream,
    config: PlaybackConfig,
}

impl PlaybackStream {
    pub fn config(&self) -> &PlaybackConfig {
        &self.config
    }
}

/// Oeffnet einen Playback-Stream auf dem gegebenen Geraet.
///
/// Gibt den Stream und den Ring-Buffer Producer zurueck.
/// Der Consumer laeuft im cpal-Callback-Thread.
pub fn open_playback_stream(
    device: &Device,
    config: PlaybackConfig,
) -> AudioResult<(PlaybackStream, PlaybackProducer)> {
    let stream_config = StreamConfig {
        channels: config.channels,
        sample_rate: cpal::SampleRate(config.sample_rate),
        buffer_size: cpal::BufferSize::Default,
    };

    let rb = HeapRb::<f32>::new(config.buffer_size);
    let (producer, mut consumer) = rb.split();

    let err_fn = |err| error!("Playback-Fehler: {}", err);

    let supported = device
        .supported_output_configs()
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
            .build_output_stream(
                &stream_config,
                move |data: &mut [f32], _| {
                    let read = consumer.pop_slice(data);
                    // Stille fuer fehlende Samples
                    if read < data.len() {
                        warn!("Playback Underrun: {} Samples fehlen", data.len() - read);
                        data[read..].fill(0.0);
                    }
                },
                err_fn,
                None,
            )
            .map_err(|e| AudioError::StreamFehler(e.to_string()))?,
        SampleFormat::I16 => device
            .build_output_stream(
                &stream_config,
                move |data: &mut [i16], _| {
                    let mut float_buf = vec![0.0f32; data.len()];
                    let read = consumer.pop_slice(&mut float_buf);
                    if read < data.len() {
                        warn!("Playback Underrun");
                    }
                    for (out, s) in data.iter_mut().zip(float_buf.iter()) {
                        *out = (*s * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32) as i16;
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
        "Playback-Stream geoeffnet: {}Hz {}ch",
        config.sample_rate, config.channels
    );

    Ok((PlaybackStream { _stream: stream, config }, producer))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cpal::traits::HostTrait;

    #[test]
    fn playback_config_default() {
        let config = PlaybackConfig::default();
        assert_eq!(config.sample_rate, 48000);
        assert_eq!(config.channels, 1);
        assert!(config.buffer_size > 0);
    }

    #[test]
    #[ignore = "Benoetigt Audio-Hardware"]
    fn playback_stream_oeffnen() {
        let host = cpal::default_host();
        if let Some(device) = host.default_output_device() {
            let config = PlaybackConfig::default();
            let result = open_playback_stream(&device, config);
            assert!(result.is_ok(), "Playback-Stream sollte oeffenbar sein");
        }
    }
}
