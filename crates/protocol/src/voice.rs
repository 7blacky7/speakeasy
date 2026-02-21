//! Voice-Protokoll (UDP)
//!
//! Definiert die binaere Paketstruktur fuer die Audio-Uebertragung via UDP.
//! Das Opus-Encoding erfolgt im Client; der Server leitet Pakete weiter (SFU-Stil).
//!
//! ## Paketformat (Header = 16 Bytes, kein serde)
//!
//! ```text
//! Offset  Len  Beschreibung
//! ------  ---  -----------
//!  0       1   Version
//!  1       1   PacketType (0 = Audio, 1 = Silence, 2 = FEC)
//!  2       2   Flags (big-endian)
//!  4       4   SequenzNummer (big-endian)
//!  8       4   Zeitstempel (big-endian, 48 kHz-Ticks)
//! 12       4   SSRC – Synchronisation Source (big-endian)
//! 16+      N   Nutzdaten (Opus-Bytes)
//! ```

use std::io;

/// Aktuelle Protokollversion
pub const PROTOKOLL_VERSION: u8 = 1;

/// Maximale Nutzdaten-Laenge (1280 Bytes, typisches Opus-MTU-Limit)
pub const MAX_NUTZDATEN_LAENGE: usize = 1280;

// ---------------------------------------------------------------------------
// Flags (u16, big-endian)
// ---------------------------------------------------------------------------

/// Bit-Masken fuer das Flags-Feld im Voice-Paket-Header
pub struct VoiceFlags;

impl VoiceFlags {
    /// Paket ist DTLS/E2E verschluesselt
    pub const ENCRYPTED: u16 = 0x0001;
    /// Paket enthaelt Forward Error Correction Daten
    pub const FEC: u16 = 0x0002;
    /// Discontinuous Transmission – Silence-Paket
    pub const DTX: u16 = 0x0004;
    /// KeyFrame fuer E2E-Verschluesselung
    pub const KEY_FRAME: u16 = 0x0008;
    /// Beginn einer Sprechsequenz
    pub const SPEAKING_START: u16 = 0x0010;
    /// Ende einer Sprechsequenz
    pub const SPEAKING_STOP: u16 = 0x0020;
}

// ---------------------------------------------------------------------------
// PacketType
// ---------------------------------------------------------------------------

/// Art des Voice-Paketes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PacketType {
    /// Normales Opus-Audio-Paket
    Audio = 0,
    /// Silence / Comfort-Noise (DTX)
    Silence = 1,
    /// Forward Error Correction Daten
    Fec = 2,
}

impl PacketType {
    /// Konvertiert ein Byte in einen `PacketType`.
    pub fn from_u8(byte: u8) -> Option<Self> {
        match byte {
            0 => Some(Self::Audio),
            1 => Some(Self::Silence),
            2 => Some(Self::Fec),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// VoicePacketHeader
// ---------------------------------------------------------------------------

/// 16-Byte Header eines Voice-UDP-Pakets
///
/// Direkte Byte-Serialisierung, kein serde (Performance-kritisch).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoicePacketHeader {
    /// Protokollversion (muss == `PROTOKOLL_VERSION` sein)
    pub version: u8,
    /// Pakettyp
    pub packet_type: PacketType,
    /// Flags-Bitmask (siehe `VoiceFlags`)
    pub flags: u16,
    /// Monoton steigende Sequenznummer (fuer Jitter-Buffer)
    pub sequence: u32,
    /// RTP-kompatibler Zeitstempel (48 kHz-Ticks)
    pub timestamp: u32,
    /// Synchronisation Source – eindeutige Senderkennung
    pub ssrc: u32,
}

impl VoicePacketHeader {
    /// Header-Groesse in Bytes
    pub const SIZE: usize = 16;

    /// Erstellt einen neuen Header
    pub fn new(
        packet_type: PacketType,
        flags: u16,
        sequence: u32,
        timestamp: u32,
        ssrc: u32,
    ) -> Self {
        Self {
            version: PROTOKOLL_VERSION,
            packet_type,
            flags,
            sequence,
            timestamp,
            ssrc,
        }
    }

    /// Serialisiert den Header in ein 16-Byte-Array (big-endian)
    pub fn encode(&self) -> [u8; Self::SIZE] {
        let mut buf = [0u8; Self::SIZE];
        buf[0] = self.version;
        buf[1] = self.packet_type as u8;
        buf[2..4].copy_from_slice(&self.flags.to_be_bytes());
        buf[4..8].copy_from_slice(&self.sequence.to_be_bytes());
        buf[8..12].copy_from_slice(&self.timestamp.to_be_bytes());
        buf[12..16].copy_from_slice(&self.ssrc.to_be_bytes());
        buf
    }

    /// Deserialisiert einen Header aus einem Byte-Slice
    ///
    /// # Fehler
    /// - `InvalidData` wenn das Slice kuerzer als 16 Bytes ist
    /// - `InvalidData` bei ungueltiger Version oder unbekanntem PacketType
    pub fn decode(buf: &[u8]) -> io::Result<Self> {
        if buf.len() < Self::SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Header zu kurz: {} Bytes (erwartet {})",
                    buf.len(),
                    Self::SIZE
                ),
            ));
        }

        let version = buf[0];
        if version != PROTOKOLL_VERSION {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Ungueltige Protokollversion: {} (erwartet {})",
                    version, PROTOKOLL_VERSION
                ),
            ));
        }

        let packet_type = PacketType::from_u8(buf[1]).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Unbekannter PacketType: {}", buf[1]),
            )
        })?;

        let flags = u16::from_be_bytes([buf[2], buf[3]]);
        let sequence = u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]);
        let timestamp = u32::from_be_bytes([buf[8], buf[9], buf[10], buf[11]]);
        let ssrc = u32::from_be_bytes([buf[12], buf[13], buf[14], buf[15]]);

        Ok(Self {
            version,
            packet_type,
            flags,
            sequence,
            timestamp,
            ssrc,
        })
    }

    /// Prueft ob ein bestimmtes Flag gesetzt ist
    pub fn hat_flag(&self, flag: u16) -> bool {
        self.flags & flag != 0
    }
}

// ---------------------------------------------------------------------------
// VoicePacket
// ---------------------------------------------------------------------------

/// Vollstaendiges Voice-UDP-Paket (Header + Opus-Nutzdaten)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoicePacket {
    /// 16-Byte Header
    pub header: VoicePacketHeader,
    /// Opus-Nutzdaten (max. `MAX_NUTZDATEN_LAENGE` Bytes)
    pub payload: Vec<u8>,
}

impl VoicePacket {
    /// Erstellt ein normales Opus-Audio-Paket
    pub fn neu_audio(sequence: u32, timestamp: u32, ssrc: u32, payload: Vec<u8>) -> Self {
        Self {
            header: VoicePacketHeader::new(PacketType::Audio, 0, sequence, timestamp, ssrc),
            payload,
        }
    }

    /// Erstellt ein Silence/DTX-Paket
    pub fn neu_silence(sequence: u32, timestamp: u32, ssrc: u32) -> Self {
        Self {
            header: VoicePacketHeader::new(
                PacketType::Silence,
                VoiceFlags::DTX,
                sequence,
                timestamp,
                ssrc,
            ),
            payload: Vec::new(),
        }
    }

    /// Serialisiert das gesamte Paket in einen Byte-Vec
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(VoicePacketHeader::SIZE + self.payload.len());
        buf.extend_from_slice(&self.header.encode());
        buf.extend_from_slice(&self.payload);
        buf
    }

    /// Deserialisiert ein Paket aus einem Byte-Slice und validiert es
    ///
    /// # Fehler
    /// - Header-Validierungsfehler (Version, PacketType)
    /// - Nutzdaten ueberschreiten `MAX_NUTZDATEN_LAENGE`
    pub fn decode(buf: &[u8]) -> io::Result<Self> {
        let header = VoicePacketHeader::decode(buf)?;
        let payload_bytes = &buf[VoicePacketHeader::SIZE..];

        if payload_bytes.len() > MAX_NUTZDATEN_LAENGE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Nutzdaten zu lang: {} Bytes (Maximum {})",
                    payload_bytes.len(),
                    MAX_NUTZDATEN_LAENGE
                ),
            ));
        }

        Ok(Self {
            header,
            payload: payload_bytes.to_vec(),
        })
    }

    /// Gesamtgroesse des Paketes in Bytes
    pub fn groesse(&self) -> usize {
        VoicePacketHeader::SIZE + self.payload.len()
    }

    /// Prueft ob die Sprachaktivitaet beginnt
    pub fn spricht_start(&self) -> bool {
        self.header.hat_flag(VoiceFlags::SPEAKING_START)
    }

    /// Prueft ob die Sprachaktivitaet endet
    pub fn spricht_stop(&self) -> bool {
        self.header.hat_flag(VoiceFlags::SPEAKING_STOP)
    }
}

// ---------------------------------------------------------------------------
// Veralteter VoicePaket-Typ (Rueckwaertskompatibilitaet Phase 1)
// ---------------------------------------------------------------------------

use serde::{Deserialize, Serialize};
use speakeasy_core::types::{ChannelId, UserId};

/// Codec fuer Audio-Pakete (Signaling-Ebene)
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

/// High-Level Voice-Paket fuer Signaling (serde-kompatibel)
///
/// Wird intern verwendet um Metadaten ueber Pakete auszutauschen.
/// Fuer den eigentlichen UDP-Transport wird `VoicePacket` verwendet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoicePaket {
    /// Absender
    pub user_id: UserId,
    /// Ziel-Kanal
    pub kanal_id: ChannelId,
    /// Monoton steigender Sequenzzaehler (fuer Jitter Buffer)
    pub sequenz: u32,
    /// RTP-kompatibler Zeitstempel
    pub zeitstempel: u32,
    /// Verwendeter Codec
    pub codec: AudioCodec,
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
            nutzdaten,
        }
    }

    /// Gibt die Nutzdaten-Laenge in Bytes zurueck
    pub fn nutzdaten_laenge(&self) -> usize {
        self.nutzdaten.len()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_encode_decode_round_trip() {
        let header = VoicePacketHeader::new(PacketType::Audio, VoiceFlags::FEC, 42, 6720, 0xDEAD);
        let encoded = header.encode();
        assert_eq!(encoded.len(), VoicePacketHeader::SIZE);
        let decoded = VoicePacketHeader::decode(&encoded).expect("Decode muss erfolgreich sein");
        assert_eq!(header, decoded);
    }

    #[test]
    fn header_groesse_ist_16_bytes() {
        let header =
            VoicePacketHeader::new(PacketType::Audio, 0, 0, 0, 0);
        assert_eq!(header.encode().len(), 16);
    }

    #[test]
    fn header_big_endian_byte_reihenfolge() {
        let header =
            VoicePacketHeader::new(PacketType::Audio, 0x0102, 0x01020304, 0x05060708, 0x090A0B0C);
        let bytes = header.encode();
        // Flags bei Offset 2-3
        assert_eq!(bytes[2], 0x01);
        assert_eq!(bytes[3], 0x02);
        // Sequence bei Offset 4-7
        assert_eq!(bytes[4], 0x01);
        assert_eq!(bytes[7], 0x04);
        // Timestamp bei Offset 8-11
        assert_eq!(bytes[8], 0x05);
        assert_eq!(bytes[11], 0x08);
        // SSRC bei Offset 12-15
        assert_eq!(bytes[12], 0x09);
        assert_eq!(bytes[15], 0x0C);
    }

    #[test]
    fn header_decode_falsche_version() {
        let mut bytes = VoicePacketHeader::new(PacketType::Audio, 0, 1, 0, 0).encode();
        bytes[0] = 99; // Ungueltige Version
        let result = VoicePacketHeader::decode(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn header_decode_zu_kurz() {
        let bytes = [0u8; 8]; // Nur 8 Bytes statt 16
        let result = VoicePacketHeader::decode(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn header_decode_unbekannter_packet_type() {
        let mut bytes = VoicePacketHeader::new(PacketType::Audio, 0, 0, 0, 0).encode();
        bytes[1] = 255; // Unbekannter Typ
        let result = VoicePacketHeader::decode(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn voice_packet_encode_decode_round_trip() {
        let payload = vec![0xAB; 120];
        let paket = VoicePacket::neu_audio(100, 4800, 0xCAFE, payload.clone());
        let encoded = paket.encode();
        assert_eq!(encoded.len(), VoicePacketHeader::SIZE + 120);

        let decoded = VoicePacket::decode(&encoded).expect("Decode muss erfolgreich sein");
        assert_eq!(decoded.header, paket.header);
        assert_eq!(decoded.payload, payload);
    }

    #[test]
    fn voice_packet_silence_hat_dtx_flag() {
        let paket = VoicePacket::neu_silence(5, 240, 0x1234);
        assert!(paket.header.hat_flag(VoiceFlags::DTX));
        assert_eq!(paket.header.packet_type, PacketType::Silence);
        assert!(paket.payload.is_empty());
    }

    #[test]
    fn voice_packet_zu_grosse_nutzdaten() {
        // Manuell ein zu grosses Paket bauen
        let header = VoicePacketHeader::new(PacketType::Audio, 0, 0, 0, 0);
        let mut buf = header.encode().to_vec();
        buf.extend(vec![0u8; MAX_NUTZDATEN_LAENGE + 1]);
        let result = VoicePacket::decode(&buf);
        assert!(result.is_err());
    }

    #[test]
    fn voice_packet_leere_nutzdaten_ok() {
        let paket = VoicePacket::neu_audio(0, 0, 0, vec![]);
        let encoded = paket.encode();
        assert_eq!(encoded.len(), VoicePacketHeader::SIZE);
        let decoded = VoicePacket::decode(&encoded).unwrap();
        assert!(decoded.payload.is_empty());
    }

    #[test]
    fn flags_kombination() {
        let flags = VoiceFlags::ENCRYPTED | VoiceFlags::FEC;
        let header = VoicePacketHeader::new(PacketType::Audio, flags, 0, 0, 0);
        assert!(header.hat_flag(VoiceFlags::ENCRYPTED));
        assert!(header.hat_flag(VoiceFlags::FEC));
        assert!(!header.hat_flag(VoiceFlags::DTX));
        assert!(!header.hat_flag(VoiceFlags::SPEAKING_START));
    }

    // --- Rueckwaertskompatibilitaet VoicePaket ---

    #[test]
    fn voice_paket_erstellen() {
        let uid = UserId::new();
        let cid = ChannelId::new();
        let paket = VoicePaket::neu_opus(uid, cid, 1, 160, vec![0xAB; 60]);
        assert_eq!(paket.codec, AudioCodec::Opus);
        assert_eq!(paket.nutzdaten_laenge(), 60);
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
