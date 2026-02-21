//! Wire-Format fuer TCP-Verbindungen
//!
//! Frame-basiertes Protokoll: Length(u32 big-endian) + JSON-Payload.
//!
//! ## Frame-Format
//!
//! ```text
//! +--------+--------+--------+--------+----...----+
//! | Laenge (u32 BE) | 4 Bytes        | Payload    |
//! +--------+--------+--------+--------+----...----+
//! ```
//!
//! Die Laenge gibt die Anzahl der Payload-Bytes an (ohne die 4 Laengen-Bytes).
//! Maximale Frame-Groesse ist konfigurierbar (Standard: 1 MB).

use bytes::{Buf, BufMut, BytesMut};
use std::io;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio_util::codec::{Decoder, Encoder};

use crate::control::ControlMessage;

// ---------------------------------------------------------------------------
// Konstanten
// ---------------------------------------------------------------------------

/// Standard-maximale Frame-Groesse (1 MB)
pub const DEFAULT_MAX_FRAME_SIZE: usize = 1024 * 1024;

/// Groesse des Laengen-Felds in Bytes
pub const LENGTH_FIELD_SIZE: usize = 4;

// ---------------------------------------------------------------------------
// FrameCodec
// ---------------------------------------------------------------------------

/// tokio-util Codec fuer frame-basierte TCP-Verbindungen
///
/// Implementiert `Encoder<ControlMessage>` und `Decoder` fuer nahtlose
/// Integration mit `tokio_util::codec::Framed`.
///
/// # Beispiel
///
/// ```rust,no_run
/// use tokio_util::codec::Framed;
/// use speakeasy_protocol::wire::FrameCodec;
///
/// // let stream = TcpStream::connect(...).await?;
/// // let framed = Framed::new(stream, FrameCodec::new());
/// ```
#[derive(Debug, Clone)]
pub struct FrameCodec {
    /// Maximale erlaubte Frame-Groesse in Bytes
    max_frame_size: usize,
}

impl FrameCodec {
    /// Erstellt einen neuen `FrameCodec` mit Standard-Limits
    pub fn new() -> Self {
        Self {
            max_frame_size: DEFAULT_MAX_FRAME_SIZE,
        }
    }

    /// Erstellt einen `FrameCodec` mit benutzerdefinierter maximaler Frame-Groesse
    pub fn with_max_size(max_frame_size: usize) -> Self {
        Self { max_frame_size }
    }

    /// Gibt die konfigurierte maximale Frame-Groesse zurueck
    pub fn max_frame_size(&self) -> usize {
        self.max_frame_size
    }
}

impl Default for FrameCodec {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Decoder-Implementierung
// ---------------------------------------------------------------------------

impl Decoder for FrameCodec {
    type Item = ControlMessage;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        // Warte auf mindestens 4 Bytes fuer das Laengen-Feld
        if src.len() < LENGTH_FIELD_SIZE {
            return Ok(None);
        }

        // Laenge lesen (big-endian u32) ohne den Buffer zu veraendern
        let length = u32::from_be_bytes([src[0], src[1], src[2], src[3]]) as usize;

        // Maximale Frame-Groesse pruefen
        if length > self.max_frame_size {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Frame zu gross: {} Bytes (Maximum: {} Bytes)",
                    length, self.max_frame_size
                ),
            ));
        }

        // Pruefen ob der vollstaendige Frame bereits im Buffer ist
        let total_size = LENGTH_FIELD_SIZE + length;
        if src.len() < total_size {
            // Speicher vorbelegen um Reallocations zu vermeiden
            src.reserve(total_size - src.len());
            return Ok(None);
        }

        // Laengen-Feld verbrauchen
        src.advance(LENGTH_FIELD_SIZE);

        // Payload-Bytes extrahieren
        let payload = src.split_to(length);

        // JSON deserialisieren
        let message: ControlMessage = serde_json::from_slice(&payload).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("JSON-Deserialisierung fehlgeschlagen: {}", e),
            )
        })?;

        Ok(Some(message))
    }
}

// ---------------------------------------------------------------------------
// Encoder-Implementierung
// ---------------------------------------------------------------------------

impl Encoder<ControlMessage> for FrameCodec {
    type Error = io::Error;

    fn encode(&mut self, item: ControlMessage, dst: &mut BytesMut) -> Result<(), Self::Error> {
        // JSON serialisieren
        let json = serde_json::to_vec(&item).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("JSON-Serialisierung fehlgeschlagen: {}", e),
            )
        })?;

        // Groesse pruefen
        if json.len() > self.max_frame_size {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Nachricht zu gross: {} Bytes (Maximum: {} Bytes)",
                    json.len(),
                    self.max_frame_size
                ),
            ));
        }

        // Laengen-Feld + Payload schreiben
        dst.reserve(LENGTH_FIELD_SIZE + json.len());
        dst.put_u32(json.len() as u32);
        dst.put_slice(&json);

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Hilfsfunktionen fuer direktes async Lesen/Schreiben
// ---------------------------------------------------------------------------

/// Liest einen einzelnen Frame aus einem `AsyncRead`
///
/// # Fehler
/// - `UnexpectedEof` wenn die Verbindung vor Abschluss des Frames getrennt wird
/// - `InvalidData` bei ungueltigem JSON oder zu grossem Frame
pub async fn read_frame<R>(reader: &mut R, max_frame_size: usize) -> io::Result<ControlMessage>
where
    R: AsyncRead + Unpin,
{
    // Laengen-Feld lesen
    let mut len_buf = [0u8; LENGTH_FIELD_SIZE];
    reader.read_exact(&mut len_buf).await?;
    let length = u32::from_be_bytes(len_buf) as usize;

    // Groesse pruefen
    if length > max_frame_size {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Frame zu gross: {} Bytes (Maximum: {} Bytes)",
                length, max_frame_size
            ),
        ));
    }

    // Payload lesen
    let mut payload = vec![0u8; length];
    reader.read_exact(&mut payload).await?;

    // JSON deserialisieren
    serde_json::from_slice(&payload).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("JSON-Deserialisierung fehlgeschlagen: {}", e),
        )
    })
}

/// Schreibt einen einzelnen Frame in einen `AsyncWrite`
///
/// # Fehler
/// - `InvalidData` wenn die Nachricht nicht serialisiert werden kann oder zu gross ist
/// - IO-Fehler beim Schreiben
pub async fn write_frame<W>(
    writer: &mut W,
    message: &ControlMessage,
    max_frame_size: usize,
) -> io::Result<()>
where
    W: AsyncWrite + Unpin,
{
    // JSON serialisieren
    let json = serde_json::to_vec(message).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("JSON-Serialisierung fehlgeschlagen: {}", e),
        )
    })?;

    // Groesse pruefen
    if json.len() > max_frame_size {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Nachricht zu gross: {} Bytes (Maximum: {} Bytes)",
                json.len(),
                max_frame_size
            ),
        ));
    }

    // Laengen-Feld + Payload schreiben
    let len_bytes = (json.len() as u32).to_be_bytes();
    writer.write_all(&len_bytes).await?;
    writer.write_all(&json).await?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::control::ControlPayload;
    use tokio_util::codec::{Decoder, Encoder};

    fn test_ping_nachricht(request_id: u32) -> ControlMessage {
        ControlMessage::ping(request_id, 999888777)
    }

    #[test]
    fn frame_codec_encode_decode_round_trip() {
        let mut codec = FrameCodec::new();
        let original = test_ping_nachricht(42);

        // Kodieren
        let mut buf = BytesMut::new();
        codec.encode(original.clone(), &mut buf).unwrap();

        // Laengen-Feld pruefen
        let payload_len = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
        assert!(payload_len > 0);
        assert_eq!(buf.len(), LENGTH_FIELD_SIZE + payload_len);

        // Dekodieren
        let decoded = codec
            .decode(&mut buf)
            .unwrap()
            .expect("Muss eine Nachricht enthalten");
        assert_eq!(decoded.request_id, 42);
        assert!(matches!(decoded.payload, ControlPayload::Ping(_)));
    }

    #[test]
    fn frame_codec_unvollstaendiger_frame() {
        let mut codec = FrameCodec::new();
        let original = test_ping_nachricht(1);

        let mut buf = BytesMut::new();
        codec.encode(original, &mut buf).unwrap();

        // Nur die Haelfte der Bytes behalten
        let half = buf.len() / 2;
        let mut partial = buf.split_to(half);

        // Sollte None zurueckgeben (wartet auf mehr Daten)
        let result = codec.decode(&mut partial).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn frame_codec_zu_wenig_bytes_fuer_laengenfeld() {
        let mut codec = FrameCodec::new();
        let mut buf = BytesMut::from(&[0x00, 0x00][..]);
        let result = codec.decode(&mut buf).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn frame_codec_ablehnung_zu_grosser_frame() {
        let mut codec = FrameCodec::with_max_size(100);

        // Frame-Laenge von 200 Bytes im Buffer simulieren
        let mut buf = BytesMut::new();
        buf.put_u32(200); // 200 Bytes Payload
        buf.put_slice(&[b'x'; 200]);

        let result = codec.decode(&mut buf);
        assert!(result.is_err());
    }

    #[test]
    fn frame_codec_ablehnung_beim_encode_zu_grosse_nachricht() {
        // Kleines Limit setzen
        let mut codec = FrameCodec::with_max_size(10);
        let original = test_ping_nachricht(1); // JSON ist sicher > 10 Bytes

        let mut buf = BytesMut::new();
        let result = codec.encode(original, &mut buf);
        assert!(result.is_err());
    }

    #[test]
    fn frame_codec_mehrere_nachrichten_im_buffer() {
        let mut codec = FrameCodec::new();
        let mut buf = BytesMut::new();

        // Drei Nachrichten kodieren
        for i in 0..3u32 {
            codec.encode(test_ping_nachricht(i), &mut buf).unwrap();
        }

        // Alle drei dekodieren
        for i in 0..3u32 {
            let msg = codec.decode(&mut buf).unwrap().expect("Nachricht erwartet");
            assert_eq!(msg.request_id, i);
        }

        // Buffer muss leer sein
        assert!(buf.is_empty());
    }

    #[test]
    fn frame_codec_default_max_size() {
        let codec = FrameCodec::new();
        assert_eq!(codec.max_frame_size(), DEFAULT_MAX_FRAME_SIZE);
    }

    #[tokio::test]
    async fn async_read_write_frame_round_trip() {
        let original = test_ping_nachricht(99);

        // In-Memory Buffer verwenden
        let mut buffer: Vec<u8> = Vec::new();
        write_frame(&mut buffer, &original, DEFAULT_MAX_FRAME_SIZE)
            .await
            .unwrap();

        assert!(buffer.len() > LENGTH_FIELD_SIZE);

        // Aus dem Buffer lesen
        let mut cursor = io::Cursor::new(buffer);
        let decoded = read_frame(&mut cursor, DEFAULT_MAX_FRAME_SIZE)
            .await
            .unwrap();

        assert_eq!(decoded.request_id, 99);
        if let ControlPayload::Ping(p) = decoded.payload {
            assert_eq!(p.timestamp_ms, 999888777);
        } else {
            panic!("Erwartet Ping-Payload");
        }
    }

    #[tokio::test]
    async fn async_read_frame_ablehnung_zu_grosser_frame() {
        // Kleines Limit, grosse Laenge
        let mut buffer: Vec<u8> = Vec::new();
        // Laengen-Feld: 2 MB
        buffer.extend_from_slice(&(2u32 * 1024 * 1024).to_be_bytes());

        let mut cursor = io::Cursor::new(buffer);
        let result = read_frame(&mut cursor, DEFAULT_MAX_FRAME_SIZE).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn async_write_frame_ablehnung_zu_grosse_nachricht() {
        let original = test_ping_nachricht(1);
        let mut buffer: Vec<u8> = Vec::new();
        let result = write_frame(&mut buffer, &original, 5).await; // Limit: 5 Bytes
        assert!(result.is_err());
    }
}
