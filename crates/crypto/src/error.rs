//! Fehlertypen fuer das Kryptografie-Subsystem

use thiserror::Error;

/// Fehler im Kryptografie-Subsystem
#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("Schluessel-Generierung fehlgeschlagen: {0}")]
    SchluesselGenerierung(String),

    #[error("Key-Exchange fehlgeschlagen: {0}")]
    KeyExchange(String),

    #[error("Verschluesselung fehlgeschlagen: {0}")]
    Verschluesselung(String),

    #[error("Entschluesselung fehlgeschlagen: {0}")]
    Entschluesselung(String),

    #[error("Signierung fehlgeschlagen: {0}")]
    Signierung(String),

    #[error("Signatur-Verifikation fehlgeschlagen: {0}")]
    SignaturVerifikation(String),

    #[error("Ungueltige Nonce-Laenge: erwartet {erwartet}, erhalten {erhalten}")]
    UngueltigeNonce { erwartet: usize, erhalten: usize },

    #[error("Ungueltige Schluessel-Laenge: erwartet {erwartet}, erhalten {erhalten}")]
    UngueltigeSchluesselLaenge { erwartet: usize, erhalten: usize },

    #[error("Ungueltige Daten: {0}")]
    UngueltigeDaten(String),

    #[error("Epoch-Mismatch: erwartet {erwartet}, erhalten {erhalten}")]
    EpochMismatch { erwartet: u32, erhalten: u32 },

    #[error("Kein Schluessel fuer Kanal {channel_id} (Epoch {epoch})")]
    KeinSchluessel { channel_id: String, epoch: u32 },

    #[error("Schluessel widerrufen (Epoch {epoch})")]
    SchluesselWiderrufen { epoch: u32 },

    #[error("Zertifikat-Generierung fehlgeschlagen: {0}")]
    ZertifikatGenerierung(String),

    #[error("TLS-Fehler: {0}")]
    Tls(String),

    #[error("Key Derivation fehlgeschlagen: {0}")]
    KeyDerivation(String),

    #[error("Base64-Dekodierung fehlgeschlagen: {0}")]
    Base64(#[from] base64::DecodeError),

    #[error("IO-Fehler: {0}")]
    Io(#[from] std::io::Error),

    #[error("Unerwarteter Fehler: {0}")]
    Anyhow(#[from] anyhow::Error),
}

pub type CryptoResult<T> = Result<T, CryptoError>;
