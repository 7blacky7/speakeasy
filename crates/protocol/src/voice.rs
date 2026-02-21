//! Voice-Protokoll-Pakete
//!
//! Definiert die UDP-Paketstruktur fuer die Audio-Uebertragung.
//! Das eigentliche Opus-Encoding erfolgt im Client; der Server
//! leitet die Pakete weiter (SFU-Stil).

use serde::{Deserialize, Serialize};
use speakeasy_core::types::{ChannelId, UserId};

/// Codec fuer Audio-Pakete
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioCodec {
    /// Opus – Standard-Codec fuer Sprache
    Opus,
    /// PCMU (G.711) – Fallback fuer maximale Kompatibilitaet
    Pcmu,
}

impl Default for AudioCodec {
    fn default() -> Self {
        Self::Opus
    }
}

/// Flags fuer ein Voice-Paket
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoiceFlags(pub u8);

impl VoiceFlags {
    /// Paket enthaelt Forward Error Correction Daten
    pub const FEC: Self = Self(0x01);
    /// Paket ist ein Silence-Frame (Comfort Noise)
    pub const SILENCE: Self = Self(0x02);
    /// Paket ist das letzte in einer Sprechsequenz
    pub const END_OF_TALK: Self = Self(0x04);

    pub fn contains(&self, flag: Self) -> bool {
        self.0 & flag.0 != 0
    }
}

impl Default for VoiceFlags {
    fn default() -> Self {
        Self(0)
    }
}

/// Ein einzelnes Voice-UDP-Paket
///
/// Wird vom Client gesendet und vom Media Router an alle anderen
/// Kanal-Teilnehmer weitergeleitet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoicePaket {
    /// Absender
    pub user_id: UserId,
    /// Ziel-Kanal
    pub kanal_id: ChannelId,
    /// Monoton steigender Sequenzzaehler (fuer Jitter Buffer)
    pub sequenz: u32,
    /// RTP-kompatibler Zeitstempel (Abtastrate abhaengig vom Codec)
    pub zeitstempel: u32,
    /// Verwendeter Codec
    pub codec: AudioCodec,
    /// Paket-Flags (FEC, Silence, End-of-Talk)
    pub flags: VoiceFlags,
    /// Rohe Codec-Nutzdaten
    pub nutzdaten: Vec<u8>,
}

impl VoicePaket {
    /// Erstellt ein neues Opus-Voice-Paket
    pub fn neu_opus(
        user_id: UserId,
        kanal_id: ChannelId,
        sequenz: u32,
        zeitstempel: u32,
        nutzdaten: Vec<u8>,
    ) -> Self {
        Self {
            user_id,
            kanal_id,
            sequenz,
            zeitstempel,
            codec: AudioCodec::Opus,
            flags: VoiceFlags::default(),
            nutzdaten,
        }
    }

    /// Gibt die Nutzdaten-Laenge in Bytes zurueck
    pub fn nutzdaten_laenge(&self) -> usize {
        self.nutzdaten.len()
    }

    /// Prueft ob dieses Paket ein Silence-Frame ist
    pub fn ist_silence(&self) -> bool {
        self.flags.contains(VoiceFlags::SILENCE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voice_paket_erstellen() {
        let uid = UserId::new();
        let cid = ChannelId::new();
        let paket = VoicePaket::neu_opus(uid, cid, 1, 160, vec![0xAB; 60]);
        assert_eq!(paket.codec, AudioCodec::Opus);
        assert_eq!(paket.nutzdaten_laenge(), 60);
        assert!(!paket.ist_silence());
    }

    #[test]
    fn voice_flags_bitmask() {
        let flags = VoiceFlags(VoiceFlags::FEC.0 | VoiceFlags::SILENCE.0);
        assert!(flags.contains(VoiceFlags::FEC));
        assert!(flags.contains(VoiceFlags::SILENCE));
        assert!(!flags.contains(VoiceFlags::END_OF_TALK));
    }

    #[test]
    fn voice_paket_serde() {
        let uid = UserId::new();
        let cid = ChannelId::new();
        let paket = VoicePaket::neu_opus(uid, cid, 42, 6720, vec![1, 2, 3]);
        let json = serde_json::to_string(&paket).unwrap();
        let paket2: VoicePaket = serde_json::from_str(&json).unwrap();
        assert_eq!(paket.sequenz, paket2.sequenz);
    }
}
