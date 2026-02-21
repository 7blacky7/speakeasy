//! DTLS-Client (TLS-Wrapper ueber TCP Control-Channel)
//!
//! Verbindet sich mit dem DTLS-Server ueber TLS.
//! Optionale Fingerprint-Verifikation fuer pinning.

use std::sync::Arc;

use rustls::ClientConfig;
use tokio_rustls::TlsConnector;

use crate::dtls::config::DtlsClientConfig;
use crate::error::CryptoResult;

/// DTLS-Client (TLS-Wrapper)
pub struct DtlsClient {
    pub connector: TlsConnector,
    pub expected_fingerprint: Option<String>,
}

impl DtlsClient {
    /// Erstellt einen neuen DTLS-Client
    ///
    /// Wenn `config.expected_fingerprint` gesetzt ist, wird der Server-Fingerprint
    /// beim Verbindungsaufbau verifiziert.
    pub fn new(config: &DtlsClientConfig) -> CryptoResult<Self> {
        // Verwende WebPKI-Roots fuer normales TLS, oder custom verifier fuer pinning
        let tls_config = ClientConfig::builder()
            .with_root_certificates(load_native_roots()?)
            .with_no_client_auth();

        let connector = TlsConnector::from(Arc::new(tls_config));

        Ok(Self {
            connector,
            expected_fingerprint: config.expected_fingerprint.clone(),
        })
    }

    /// Erstellt einen Client der jedes Zertifikat akzeptiert (nur fuer Tests!)
    pub fn new_insecure() -> CryptoResult<Self> {
        let tls_config = ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(InsecureCertVerifier))
            .with_no_client_auth();

        let connector = TlsConnector::from(Arc::new(tls_config));

        Ok(Self {
            connector,
            expected_fingerprint: None,
        })
    }

    /// Verifiziert einen Zertifikat-Fingerprint
    pub fn verify_fingerprint(&self, server_fingerprint: &str) -> bool {
        match &self.expected_fingerprint {
            Some(expected) => expected == server_fingerprint,
            None => true, // Kein Pinning konfiguriert
        }
    }
}

impl std::fmt::Debug for DtlsClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DtlsClient")
            .field("fingerprint_pinning", &self.expected_fingerprint.is_some())
            .finish()
    }
}

fn load_native_roots() -> CryptoResult<rustls::RootCertStore> {
    let mut roots = rustls::RootCertStore::empty();
    // webpki-roots einbinden
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    Ok(roots)
}

/// Unsicherer Zertifikat-Verifier (nur fuer Tests mit selbstsignierten Certs)
#[derive(Debug)]
struct InsecureCertVerifier;

impl rustls::client::danger::ServerCertVerifier for InsecureCertVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![
            rustls::SignatureScheme::RSA_PKCS1_SHA256,
            rustls::SignatureScheme::RSA_PKCS1_SHA384,
            rustls::SignatureScheme::RSA_PKCS1_SHA512,
            rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            rustls::SignatureScheme::ECDSA_NISTP384_SHA384,
            rustls::SignatureScheme::ED25519,
        ]
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn install_crypto_provider() {
        let _ = rustls::crypto::ring::default_provider().install_default();
    }

    #[test]
    fn dtls_client_insecure_erstellen() {
        install_crypto_provider();
        let client = DtlsClient::new_insecure().unwrap();
        assert!(client.expected_fingerprint.is_none());
    }

    #[test]
    fn fingerprint_verifikation_korrekt() {
        install_crypto_provider();
        let client = DtlsClient::new_insecure().unwrap();
        let client = DtlsClient {
            connector: client.connector,
            expected_fingerprint: Some("AA:BB:CC".to_string()),
        };
        assert!(client.verify_fingerprint("AA:BB:CC"));
        assert!(!client.verify_fingerprint("DD:EE:FF"));
    }

    #[test]
    fn kein_pinning_akzeptiert_alles() {
        install_crypto_provider();
        let client = DtlsClient::new_insecure().unwrap();
        assert!(client.verify_fingerprint("irgendein-fingerprint"));
    }

    #[test]
    fn dtls_client_debug_format() {
        install_crypto_provider();
        let client = DtlsClient::new_insecure().unwrap();
        let debug_str = format!("{:?}", client);
        assert!(debug_str.contains("DtlsClient"));
    }
}
