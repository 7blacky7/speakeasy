//! Opus Encoder/Decoder Wrapper
//!
//! Kapselt audiopus und stellt eine einfache f32-PCM basierte API bereit.
//! Nutzt OpusConfig aus speakeasy-protocol fuer Konfiguration.

use audiopus::{
    coder::{Decoder, Encoder},
    Application, Channels, SampleRate,
};
use tracing::debug;

use crate::error::{AudioError, AudioResult};
use speakeasy_protocol::codec::{
    ChannelCount, FrameSizeMs, OpusApplication, OpusConfig, SampleRate as ProtocolSampleRate,
};

/// Opus-Encoder: kodiert f32-PCM zu Opus-Bytes
pub struct OpusEncoder {
    encoder: Encoder,
    config: OpusConfig,
    frame_size: usize,
}

impl OpusEncoder {
    /// Erstellt einen neuen Encoder mit der gegebenen Konfiguration
    pub fn new(config: OpusConfig) -> AudioResult<Self> {
        config.validieren().map_err(AudioError::Konfiguration)?;

        let sample_rate = protocol_rate_to_audiopus(config.sample_rate)?;
        let channels = protocol_channels_to_audiopus(config.channels);
        let application = protocol_app_to_audiopus(config.application);

        let mut encoder = Encoder::new(sample_rate, channels, application)
            .map_err(|e| AudioError::CodecFehler(e.to_string()))?;

        // Bitrate setzen
        encoder
            .set_bitrate(audiopus::Bitrate::BitsPerSecond(
                (config.bitrate_kbps as i32) * 1000,
            ))
            .map_err(|e| AudioError::CodecFehler(e.to_string()))?;

        // Komplexitaet setzen (audiopus 0.2 erwartet u8)
        encoder
            .set_complexity(config.complexity)
            .map_err(|e| AudioError::CodecFehler(e.to_string()))?;

        // VBR
        encoder
            .set_vbr(config.vbr_enabled)
            .map_err(|e| AudioError::CodecFehler(e.to_string()))?;

        // FEC
        encoder
            .set_inband_fec(config.fec_enabled)
            .map_err(|e| AudioError::CodecFehler(e.to_string()))?;

        // DTX: audiopus 0.2 hat kein set_dtx – wird ueber set_prediction_disabled angenähert
        // DTX-Effekt: bei sehr leisem Signal Pakete nicht senden (hier via encoder-ctl)
        if config.dtx_enabled {
            // OpusDTX ctl: OPUS_SET_DTX_REQUEST = 4016
            let _ = encoder.set_encoder_ctl_request(4016, 1);
        }

        let frame_size = config.frame_size.samples_per_frame(config.sample_rate) as usize;

        debug!(
            "OpusEncoder erstellt: {}kbps, {:?}, frame_size={}",
            config.bitrate_kbps, config.sample_rate, frame_size
        );

        Ok(Self {
            encoder,
            config,
            frame_size,
        })
    }

    /// Kodiert einen PCM-Frame (f32, normalisiert -1.0..1.0) zu Opus-Bytes
    ///
    /// Die Eingabe muss exakt `frame_size()` Samples lang sein.
    pub fn encode(&mut self, pcm: &[f32]) -> AudioResult<Vec<u8>> {
        if pcm.len() != self.frame_size {
            return Err(AudioError::Konfiguration(format!(
                "PCM-Frame muss {} Samples lang sein, war {}",
                self.frame_size,
                pcm.len()
            )));
        }

        // Puffer: max. 4000 Bytes reicht fuer alle Opus-Frames
        let mut output = vec![0u8; 4000];
        let written = self
            .encoder
            .encode_float(pcm, &mut output)
            .map_err(|e| AudioError::CodecFehler(e.to_string()))?;

        output.truncate(written);
        Ok(output)
    }

    /// Gibt die erwartete Frame-Groesse in Samples zurueck
    pub fn frame_size(&self) -> usize {
        self.frame_size
    }

    /// Gibt die aktuelle Konfiguration zurueck
    pub fn config(&self) -> &OpusConfig {
        &self.config
    }
}

/// Opus-Decoder: dekodiert Opus-Bytes zu f32-PCM
pub struct OpusDecoder {
    decoder: Decoder,
    sample_rate: ProtocolSampleRate,
    channels: ChannelCount,
    frame_size: usize,
}

impl OpusDecoder {
    /// Erstellt einen neuen Decoder
    pub fn new(sample_rate: ProtocolSampleRate, channels: ChannelCount) -> AudioResult<Self> {
        let sr = protocol_rate_to_audiopus(sample_rate)?;
        let ch = protocol_channels_to_audiopus(channels);

        let decoder = Decoder::new(sr, ch).map_err(|e| AudioError::CodecFehler(e.to_string()))?;

        // Standardmaessig 20ms Frame-Groesse
        let frame_size = FrameSizeMs::Ms20.samples_per_frame(sample_rate) as usize;

        debug!(
            "OpusDecoder erstellt: {:?} {:?} frame_size={}",
            sample_rate, channels, frame_size
        );

        Ok(Self {
            decoder,
            sample_rate,
            channels,
            frame_size,
        })
    }

    /// Erstellt einen Decoder aus einer OpusConfig
    pub fn from_config(config: &OpusConfig) -> AudioResult<Self> {
        let mut dec = Self::new(config.sample_rate, config.channels)?;
        dec.frame_size = config.frame_size.samples_per_frame(config.sample_rate) as usize;
        Ok(dec)
    }

    /// Dekodiert Opus-Bytes zu f32-PCM
    pub fn decode(&mut self, opus_data: &[u8]) -> AudioResult<Vec<f32>> {
        let mut output = vec![0.0f32; self.frame_size * self.channels as usize];
        let decoded = self
            .decoder
            .decode_float(Some(opus_data), &mut output, false)
            .map_err(|e| AudioError::CodecFehler(e.to_string()))?;

        output.truncate(decoded * self.channels as usize);
        Ok(output)
    }

    /// Dekodiert mit PLC (Packet Loss Concealment) wenn kein Paket empfangen
    pub fn decode_plc(&mut self) -> AudioResult<Vec<f32>> {
        let mut output = vec![0.0f32; self.frame_size * self.channels as usize];
        let decoded = self
            .decoder
            .decode_float(None::<&[u8]>, &mut output, false)
            .map_err(|e| AudioError::CodecFehler(e.to_string()))?;

        output.truncate(decoded * self.channels as usize);
        Ok(output)
    }

    /// Gibt die erwartete Frame-Groesse in Samples zurueck
    pub fn frame_size(&self) -> usize {
        self.frame_size
    }

    /// Gibt die Kanalanzahl zurueck
    pub fn channels(&self) -> ChannelCount {
        self.channels
    }

    /// Gibt die Abtastrate zurueck
    pub fn sample_rate(&self) -> ProtocolSampleRate {
        self.sample_rate
    }
}

// ---------------------------------------------------------------------------
// Konvertierungs-Hilfsfunktionen
// ---------------------------------------------------------------------------

fn protocol_rate_to_audiopus(rate: ProtocolSampleRate) -> AudioResult<SampleRate> {
    match rate {
        ProtocolSampleRate::Hz8000 => Ok(SampleRate::Hz8000),
        ProtocolSampleRate::Hz12000 => Ok(SampleRate::Hz12000),
        ProtocolSampleRate::Hz16000 => Ok(SampleRate::Hz16000),
        ProtocolSampleRate::Hz24000 => Ok(SampleRate::Hz24000),
        ProtocolSampleRate::Hz48000 => Ok(SampleRate::Hz48000),
    }
}

fn protocol_channels_to_audiopus(ch: ChannelCount) -> Channels {
    match ch {
        ChannelCount::Mono => Channels::Mono,
        ChannelCount::Stereo => Channels::Stereo,
    }
}

fn protocol_app_to_audiopus(app: OpusApplication) -> Application {
    match app {
        OpusApplication::Voip => Application::Voip,
        OpusApplication::Audio => Application::Audio,
        OpusApplication::RestrictedLowdelay => Application::LowDelay,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use speakeasy_protocol::codec::{AudioPreset, ChannelCount, SampleRate as PSR};

    #[test]
    fn encoder_konfiguration_speech() {
        let config = AudioPreset::Speech.config();
        let encoder = OpusEncoder::new(config.clone());
        assert!(encoder.is_ok(), "Speech-Encoder sollte erstellbar sein");
        let enc = encoder.unwrap();
        assert_eq!(enc.config().bitrate_kbps, 32);
        // 20ms bei 16kHz = 320 Samples
        assert_eq!(enc.frame_size(), 320);
    }

    #[test]
    fn encoder_konfiguration_balanced() {
        let config = AudioPreset::Balanced.config();
        let encoder = OpusEncoder::new(config);
        assert!(encoder.is_ok());
        let enc = encoder.unwrap();
        // 20ms bei 48kHz = 960 Samples
        assert_eq!(enc.frame_size(), 960);
    }

    #[test]
    fn decoder_konfiguration_mono() {
        let dec = OpusDecoder::new(PSR::Hz48000, ChannelCount::Mono);
        assert!(dec.is_ok());
        let dec = dec.unwrap();
        assert_eq!(dec.frame_size(), 960);
        assert_eq!(dec.channels() as usize, 1);
    }

    #[test]
    fn decoder_konfiguration_stereo() {
        let dec = OpusDecoder::new(PSR::Hz48000, ChannelCount::Stereo);
        assert!(dec.is_ok());
        let dec = dec.unwrap();
        assert_eq!(dec.channels() as usize, 2);
    }

    #[test]
    fn decoder_from_config() {
        let config = AudioPreset::Music.config();
        let dec = OpusDecoder::from_config(&config);
        assert!(dec.is_ok());
    }

    #[test]
    fn encoder_falscher_frame_size_fehler() {
        let config = AudioPreset::Speech.config();
        let mut enc = OpusEncoder::new(config).unwrap();
        // 320 Samples erwartet, aber 100 uebergeben
        let result = enc.encode(&vec![0.0f32; 100]);
        assert!(result.is_err());
    }

    #[test]
    fn encoder_decoder_roundtrip() {
        let config = AudioPreset::Speech.config();
        let mut enc = OpusEncoder::new(config.clone()).unwrap();
        let mut dec = OpusDecoder::from_config(&config).unwrap();

        let frame_size = enc.frame_size();
        let pcm_in: Vec<f32> = (0..frame_size)
            .map(|i| (i as f32 / frame_size as f32 * 0.1).sin() * 0.5)
            .collect();

        let encoded = enc.encode(&pcm_in).expect("Encoding sollte funktionieren");
        assert!(!encoded.is_empty());

        let decoded = dec.decode(&encoded).expect("Decoding sollte funktionieren");
        assert_eq!(decoded.len(), frame_size);
    }

    #[test]
    fn encoder_ungueltige_konfiguration() {
        let mut config = AudioPreset::Speech.config();
        config.bitrate_kbps = 5; // Ungueltig
        let result = OpusEncoder::new(config);
        assert!(result.is_err());
    }

    #[test]
    fn alle_presets_encoder_erstellbar() {
        use speakeasy_protocol::codec::AudioPreset;
        for preset in [
            AudioPreset::Speech,
            AudioPreset::Balanced,
            AudioPreset::LowBandwidth,
        ] {
            let config = preset.config();
            let result = OpusEncoder::new(config);
            assert!(
                result.is_ok(),
                "Preset {:?} sollte Encoder erstellen koennen",
                preset
            );
        }
    }
}
