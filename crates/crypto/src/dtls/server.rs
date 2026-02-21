//! DTLS-Server (TLS-Wrapper ueber TCP Control-Channel)
//!
//! Echtes DTLS ueber UDP ist komplex (keine stabile Rust-Bibliothek ohne C-Bindungen).
//! Wir implementieren einen TLS-Wrapper ueber den TCP Control-Channel.
//! UDP-DTLS ist als TODO markiert und wird in einer spaeteren Phase implementiert.
//!
//! ## Architektur
//! - Control-Channel: TLS (tokio-rustls) ueber TCP
//! - Voice-Kanal: TODO - Echtes DTLS ueber UDP (z.B. mit openssl oder mbedtls FFI)

use std::sync::Arc;

use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::ServerConfig;
use rustls_pemfile::{certs, private_key};
use tokio_rustls::TlsAcceptor;

use crate::dtls::config::DtlsServerConfig;
use crate::error::{CryptoError, CryptoResult};

/// DTLS-Server (TLS-Wrapper)
pub struct DtlsServer {
    pub acceptor: TlsAcceptor,
    pub certificate_fingerprint: String,
}

impl DtlsServer {
    /// Erstellt einen neuen DTLS-Server aus der Konfiguration
    pub fn new(config: &DtlsServerConfig) -> CryptoResult<Self> {
        let cert_chain = parse_certificates(&config.certificate_pem)?;
        let private_key = parse_private_key(&config.private_key_pem)?;

        let tls_config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(cert_chain, private_key)
            .map_err(|e| CryptoError::Tls(e.to_string()))?;

        let acceptor = TlsAcceptor::from(Arc::new(tls_config));

        Ok(Self {
            acceptor,
            certificate_fingerprint: config.certificate_fingerprint.clone(),
        })
    }

    /// Gibt den Zertifikat-Fingerprint zurueck (fuer Client-Verifikation)
    pub fn fingerprint(&self) -> &str {
        &self.certificate_fingerprint
    }
}

impl std::fmt::Debug for DtlsServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DtlsServer")
            .field("fingerprint", &self.certificate_fingerprint)
            .finish()
    }
}

fn parse_certificates(pem: &str) -> CryptoResult<Vec<CertificateDer<'static>>> {
    let mut cursor = std::io::Cursor::new(pem.as_bytes());
    certs(&mut cursor)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| CryptoError::Tls(format!("Zertifikat-Parsing fehlgeschlagen: {}", e)))
}

fn parse_private_key(pem: &str) -> CryptoResult<PrivateKeyDer<'static>> {
    let mut cursor = std::io::Cursor::new(pem.as_bytes());
    private_key(&mut cursor)
        .map_err(|e| CryptoError::Tls(format!("Schluessel-Parsing fehlgeschlagen: {}", e)))?
        .ok_or_else(|| CryptoError::Tls("Kein privater Schluessel gefunden".to_string()))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dtls::config::generate_self_signed_cert;

    fn install_crypto_provider() {
        let _ = rustls::crypto::ring::default_provider().install_default();
    }

    #[test]
    fn dtls_server_erstellen() {
        install_crypto_provider();
        let config = generate_self_signed_cert("test-server").unwrap();
        let server = DtlsServer::new(&config).unwrap();
        assert_eq!(server.fingerprint(), config.certificate_fingerprint);
    }

    #[test]
    fn dtls_server_mit_ungueltigem_cert_schlaegt_fehl() {
        install_crypto_provider();
        let config = DtlsServerConfig {
            certificate_pem: "ungueltig".to_string(),
            private_key_pem: "ungueltig".to_string(),
            certificate_fingerprint: "AA:BB".to_string(),
        };
        let result = DtlsServer::new(&config);
        assert!(result.is_err());
    }

    #[test]
    fn dtls_server_debug_format() {
        install_crypto_provider();
        let config = generate_self_signed_cert("debug-test").unwrap();
        let server = DtlsServer::new(&config).unwrap();
        let debug_str = format!("{:?}", server);
        assert!(debug_str.contains("DtlsServer"));
    }
}
