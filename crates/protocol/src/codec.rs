//! Codec-Konfiguration fuer Audio-Uebertragung
//!
//! Definiert Opus-Konfigurationstypen und vordefinierte Audio-Presets
//! fuer verschiedene Anwendungsszenarien (Sprache, Musik, etc.).
//! Die Codec-Negotiation laeuft ueber das Control-Protokoll (TCP).

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Opus-Konfiguration
// ---------------------------------------------------------------------------

/// Abtastrate fuer Opus
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SampleRate {
    /// 8 kHz – Schmalband (Telefon-Qualitaet)
    Hz8000 = 8000,
    /// 12 kHz – Mittelband
    Hz12000 = 12000,
    /// 16 kHz – Breitband (gute Sprach-Qualitaet)
    Hz16000 = 16000,
    /// 24 kHz – Superbreitband
    Hz24000 = 24000,
    /// 48 kHz – Vollband (Standard fuer Musik und hohe Qualitaet)
    #[default]
    Hz48000 = 48000,
}

/// Anzahl der Audio-Kanaele
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelCount {
    /// Mono (1 Kanal) – fuer Sprache empfohlen
    #[default]
    Mono = 1,
    /// Stereo (2 Kanaele) – fuer Musik
    Stereo = 2,
}

/// Opus-Anwendungsmodus
///
/// Beeinflusst intern den Opus-Encoder-Algorithmus.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpusApplication {
    /// Optimiert fuer Sprachverstaendlichkeit (VOIP)
    #[default]
    Voip,
    /// Optimiert fuer allgemeine Audio-Qualitaet (Musik)
    Audio,
    /// Minimale Verarbeitungsverzoegerung (Restricted Lowdelay)
    RestrictedLowdelay,
}

/// Frame-Groesse in Millisekunden
///
/// Beeinflusst Latenz vs. Kompressionseffizienz.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrameSizeMs {
    /// 2.5 ms – minimale Latenz (als Zehntelmillisekunden: 25)
    Ms2_5 = 25,
    /// 5 ms
    Ms5 = 50,
    /// 10 ms – guter Kompromiss fuer Sprache
    Ms10 = 100,
    /// 20 ms – Standard (bester Qualitaets-/Latenz-Kompromiss)
    #[default]
    Ms20 = 200,
    /// 40 ms – hohe Kompression, mehr Latenz
    Ms40 = 400,
    /// 60 ms – maximale Kompression
    Ms60 = 600,
}

impl FrameSizeMs {
    /// Gibt die Frame-Groesse als Millisekunden zurueck
    pub fn as_ms(&self) -> f32 {
        (*self as u32) as f32 / 10.0
    }

    /// Berechnet die Anzahl der Samples pro Frame bei gegebener Abtastrate
    pub fn samples_per_frame(&self, sample_rate: SampleRate) -> u32 {
        let rate = sample_rate as u32;
        let ms_x10 = *self as u32;
        // ms_x10 / 10 * rate / 1000 = ms_x10 * rate / 10000
        ms_x10 * rate / 10000
    }
}

/// Vollstaendige Opus-Codec-Konfiguration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpusConfig {
    /// Ziel-Bitrate in kbps (6–510)
    pub bitrate_kbps: u16,
    /// Abtastrate
    pub sample_rate: SampleRate,
    /// Anzahl der Kanaele
    pub channels: ChannelCount,
    /// Frame-Groesse
    pub frame_size: FrameSizeMs,
    /// Anwendungsmodus
    pub application: OpusApplication,
    /// Forward Error Correction aktivieren
    pub fec_enabled: bool,
    /// Discontinuous Transmission (Silence-Suppression) aktivieren
    pub dtx_enabled: bool,
    /// Komplexitaet (0–10, hoeher = bessere Qualitaet, mehr CPU)
    pub complexity: u8,
    /// Variable Bitrate aktivieren
    pub vbr_enabled: bool,
}

impl OpusConfig {
    /// Validiert die Konfiguration
    pub fn validieren(&self) -> Result<(), String> {
        if self.bitrate_kbps < 6 || self.bitrate_kbps > 510 {
            return Err(format!(
                "Bitrate muss zwischen 6 und 510 kbps liegen (war: {})",
                self.bitrate_kbps
            ));
        }
        if self.complexity > 10 {
            return Err(format!(
                "Komplexitaet muss zwischen 0 und 10 liegen (war: {})",
                self.complexity
            ));
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Audio-Presets
// ---------------------------------------------------------------------------

/// Vordefinierte Audio-Konfigurationen fuer haeufige Anwendungsfaelle
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioPreset {
    /// Optimiert fuer Sprache (niedriger Bitrate, niedrige Latenz, FEC aktiv)
    Speech,
    /// Ausgewogener Kompromiss zwischen Qualitaet und Bandbreite
    Balanced,
    /// Optimiert fuer Musik (hohe Bitrate, Stereo, keine Silence-Suppression)
    Music,
    /// Minimale Bandbreite (sehr niedrige Bitrate, DTX aktiv)
    LowBandwidth,
}

impl AudioPreset {
    /// Gibt die vordefinierte `OpusConfig` fuer dieses Preset zurueck
    pub fn config(&self) -> OpusConfig {
        match self {
            AudioPreset::Speech => OpusConfig {
                bitrate_kbps: 32,
                sample_rate: SampleRate::Hz16000,
                channels: ChannelCount::Mono,
                frame_size: FrameSizeMs::Ms20,
                application: OpusApplication::Voip,
                fec_enabled: true,
                dtx_enabled: true,
                complexity: 8,
                vbr_enabled: true,
            },
            AudioPreset::Balanced => OpusConfig {
                bitrate_kbps: 64,
                sample_rate: SampleRate::Hz48000,
                channels: ChannelCount::Mono,
                frame_size: FrameSizeMs::Ms20,
                application: OpusApplication::Voip,
                fec_enabled: true,
                dtx_enabled: false,
                complexity: 9,
                vbr_enabled: true,
            },
            AudioPreset::Music => OpusConfig {
                bitrate_kbps: 192,
                sample_rate: SampleRate::Hz48000,
                channels: ChannelCount::Stereo,
                frame_size: FrameSizeMs::Ms20,
                application: OpusApplication::Audio,
                fec_enabled: false,
                dtx_enabled: false,
                complexity: 10,
                vbr_enabled: false,
            },
            AudioPreset::LowBandwidth => OpusConfig {
                bitrate_kbps: 12,
                sample_rate: SampleRate::Hz8000,
                channels: ChannelCount::Mono,
                frame_size: FrameSizeMs::Ms40,
                application: OpusApplication::Voip,
                fec_enabled: false,
                dtx_enabled: true,
                complexity: 5,
                vbr_enabled: true,
            },
        }
    }

    /// Gibt den menschenlesbaren Namen des Presets zurueck
    pub fn bezeichnung(&self) -> &'static str {
        match self {
            AudioPreset::Speech => "Sprache",
            AudioPreset::Balanced => "Ausgewogen",
            AudioPreset::Music => "Musik",
            AudioPreset::LowBandwidth => "Niedrige Bandbreite",
        }
    }
}

// ---------------------------------------------------------------------------
// Codec-Negotiation
// ---------------------------------------------------------------------------

/// Codec-Aushandlung zwischen Client und Server
///
/// Der Client sendet eine `CodecNegotiationRequest` mit der gewuenschten
/// Konfiguration. Der Server antwortet mit einer `CodecNegotiationResponse`
/// die die erlaubte (moeglicherweise angepasste) Konfiguration enthaelt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodecNegotiationRequest {
    /// Gewuenschte Konfiguration des Clients
    pub requested: OpusConfig,
    /// Optional: Preset als Kurzform (wird in `requested` aufgeloest)
    pub preset_hint: Option<AudioPreset>,
    /// Unterstuetzte Abtastraten (in Praeferenz-Reihenfolge)
    pub supported_sample_rates: Vec<SampleRate>,
    /// Maximale Bitrate die der Client senden kann (kbps)
    pub max_upload_kbps: u16,
    /// Maximale Bitrate die der Client empfangen kann (kbps)
    pub max_download_kbps: u16,
}

/// Status der Codec-Aushandlung
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NegotiationStatus {
    /// Gewuenschte Konfiguration akzeptiert
    Accepted,
    /// Konfiguration angepasst (Details in `adjusted`)
    Adjusted,
    /// Keine kompatible Konfiguration gefunden
    Rejected,
}

/// Antwort des Servers auf eine Codec-Aushandlung
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodecNegotiationResponse {
    /// Status der Aushandlung
    pub status: NegotiationStatus,
    /// Vom Server erlaubte/angepasste Konfiguration
    pub accepted: OpusConfig,
    /// Grund fuer Anpassungen (falls `status == Adjusted`)
    pub adjustment_reason: Option<String>,
    /// Maximale erlaubte Bitrate des Servers (serverseitige Begrenzung)
    pub server_max_bitrate_kbps: u16,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speech_preset_konfiguration() {
        let config = AudioPreset::Speech.config();
        assert_eq!(config.bitrate_kbps, 32);
        assert_eq!(config.sample_rate, SampleRate::Hz16000);
        assert_eq!(config.channels, ChannelCount::Mono);
        assert!(config.fec_enabled);
        assert!(config.dtx_enabled);
        assert_eq!(config.application, OpusApplication::Voip);
    }

    #[test]
    fn balanced_preset_konfiguration() {
        let config = AudioPreset::Balanced.config();
        assert_eq!(config.bitrate_kbps, 64);
        assert_eq!(config.sample_rate, SampleRate::Hz48000);
        assert!(config.fec_enabled);
        assert!(!config.dtx_enabled);
    }

    #[test]
    fn music_preset_konfiguration() {
        let config = AudioPreset::Music.config();
        assert_eq!(config.bitrate_kbps, 192);
        assert_eq!(config.channels, ChannelCount::Stereo);
        assert!(!config.fec_enabled);
        assert!(!config.dtx_enabled);
        assert_eq!(config.application, OpusApplication::Audio);
        assert!(!config.vbr_enabled);
    }

    #[test]
    fn low_bandwidth_preset_konfiguration() {
        let config = AudioPreset::LowBandwidth.config();
        assert_eq!(config.bitrate_kbps, 12);
        assert_eq!(config.sample_rate, SampleRate::Hz8000);
        assert!(config.dtx_enabled);
        assert!(!config.fec_enabled);
    }

    #[test]
    fn alle_presets_validierbar() {
        let presets = [
            AudioPreset::Speech,
            AudioPreset::Balanced,
            AudioPreset::Music,
            AudioPreset::LowBandwidth,
        ];
        for preset in &presets {
            let config = preset.config();
            assert!(
                config.validieren().is_ok(),
                "Preset {:?} hat ungueltige Konfiguration",
                preset
            );
        }
    }

    #[test]
    fn opus_config_validierung_ungueltige_bitrate() {
        let mut config = AudioPreset::Speech.config();
        config.bitrate_kbps = 5; // Zu niedrig
        assert!(config.validieren().is_err());

        config.bitrate_kbps = 511; // Zu hoch
        assert!(config.validieren().is_err());
    }

    #[test]
    fn opus_config_validierung_ungueltige_komplexitaet() {
        let mut config = AudioPreset::Balanced.config();
        config.complexity = 11; // Zu hoch
        assert!(config.validieren().is_err());
    }

    #[test]
    fn frame_size_als_ms() {
        assert!((FrameSizeMs::Ms2_5.as_ms() - 2.5).abs() < f32::EPSILON);
        assert!((FrameSizeMs::Ms20.as_ms() - 20.0).abs() < f32::EPSILON);
        assert!((FrameSizeMs::Ms60.as_ms() - 60.0).abs() < f32::EPSILON);
    }

    #[test]
    fn frame_size_samples_per_frame() {
        // 20ms bei 48kHz = 960 Samples
        let samples = FrameSizeMs::Ms20.samples_per_frame(SampleRate::Hz48000);
        assert_eq!(samples, 960);

        // 10ms bei 16kHz = 160 Samples
        let samples = FrameSizeMs::Ms10.samples_per_frame(SampleRate::Hz16000);
        assert_eq!(samples, 160);
    }

    #[test]
    fn codec_negotiation_serialisierung() {
        let req = CodecNegotiationRequest {
            requested: AudioPreset::Balanced.config(),
            preset_hint: Some(AudioPreset::Balanced),
            supported_sample_rates: vec![SampleRate::Hz48000, SampleRate::Hz16000],
            max_upload_kbps: 128,
            max_download_kbps: 256,
        };
        let json = serde_json::to_string(&req).unwrap();
        let decoded: CodecNegotiationRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.requested.bitrate_kbps, 64);
        assert_eq!(decoded.preset_hint, Some(AudioPreset::Balanced));
    }

    #[test]
    fn negotiation_response_serialisierung() {
        let resp = CodecNegotiationResponse {
            status: NegotiationStatus::Adjusted,
            accepted: AudioPreset::Speech.config(),
            adjustment_reason: Some("Bitrate auf Server-Maximum reduziert".to_string()),
            server_max_bitrate_kbps: 32,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let decoded: CodecNegotiationResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.status, NegotiationStatus::Adjusted);
        assert!(decoded.adjustment_reason.is_some());
    }

    #[test]
    fn preset_bezeichnungen_nicht_leer() {
        let presets = [
            AudioPreset::Speech,
            AudioPreset::Balanced,
            AudioPreset::Music,
            AudioPreset::LowBandwidth,
        ];
        for preset in &presets {
            assert!(!preset.bezeichnung().is_empty());
        }
    }

    #[test]
    fn opus_config_serde_round_trip() {
        let config = AudioPreset::Music.config();
        let json = serde_json::to_string(&config).unwrap();
        let decoded: OpusConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, decoded);
    }
}
