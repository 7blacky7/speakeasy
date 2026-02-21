//! DTLS-Konfiguration und Zertifikat-Generierung
//!
//! Fuer Development werden selbstsignierte Zertifikate via rcgen generiert.
//! In Produktion wuerden echte CA-Zertifikate eingesetzt.

use rcgen::{CertificateParams, DistinguishedName, KeyPair as RcgenKeyPair};

use crate::error::{CryptoError, CryptoResult};

/// DTLS-Server-Konfiguration
#[derive(Debug, Clone)]
pub struct DtlsServerConfig {
    /// PEM-kodiertes Zertifikat
    pub certificate_pem: String,
    /// PEM-kodierter privater Schluessel
    pub private_key_pem: String,
    /// SHA-256 Fingerprint des Zertifikats (fuer Client-Verifikation)
    pub certificate_fingerprint: String,
}

/// DTLS-Client-Konfiguration
#[derive(Debug, Clone)]
pub struct DtlsClientConfig {
    /// Erwarteter Zertifikat-Fingerprint des Servers (fuer Verifikation)
    pub expected_fingerprint: Option<String>,
}

impl DtlsClientConfig {
    pub fn new() -> Self {
        Self {
            expected_fingerprint: None,
        }
    }

    pub fn with_fingerprint(fingerprint: String) -> Self {
        Self {
            expected_fingerprint: Some(fingerprint),
        }
    }
}

impl Default for DtlsClientConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Generiert ein selbstsigniertes Zertifikat fuer Development/Testing
pub fn generate_self_signed_cert(common_name: &str) -> CryptoResult<DtlsServerConfig> {
    let mut params = CertificateParams::new(vec![common_name.to_string()])
        .map_err(|e| CryptoError::ZertifikatGenerierung(e.to_string()))?;

    let mut distinguished_name = DistinguishedName::new();
    distinguished_name.push(rcgen::DnType::CommonName, common_name);
    params.distinguished_name = distinguished_name;

    let key_pair =
        RcgenKeyPair::generate().map_err(|e| CryptoError::ZertifikatGenerierung(e.to_string()))?;

    let cert = params
        .self_signed(&key_pair)
        .map_err(|e| CryptoError::ZertifikatGenerierung(e.to_string()))?;

    let certificate_pem = cert.pem();
    let private_key_pem = key_pair.serialize_pem();

    // SHA-256 Fingerprint berechnen
    let fingerprint = compute_certificate_fingerprint(cert.der());

    Ok(DtlsServerConfig {
        certificate_pem,
        private_key_pem,
        certificate_fingerprint: fingerprint,
    })
}

/// Berechnet den SHA-256 Fingerprint eines DER-kodierten Zertifikats
pub fn compute_certificate_fingerprint(der_bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(der_bytes);
    hash.iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .join(":")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn self_signed_cert_generierung() {
        let config = generate_self_signed_cert("speakeasy-test").unwrap();
        assert!(!config.certificate_pem.is_empty());
        assert!(!config.private_key_pem.is_empty());
        assert!(!config.certificate_fingerprint.is_empty());

        // Fingerprint hat SHA-256 Format: XX:XX:...:XX (95 Zeichen fuer 32 Bytes)
        let parts: Vec<&str> = config.certificate_fingerprint.split(':').collect();
        assert_eq!(parts.len(), 32);
        for part in &parts {
            assert_eq!(part.len(), 2);
        }
    }

    #[test]
    fn self_signed_cert_pem_format() {
        let config = generate_self_signed_cert("test-server").unwrap();
        assert!(config
            .certificate_pem
            .contains("-----BEGIN CERTIFICATE-----"));
        assert!(config.certificate_pem.contains("-----END CERTIFICATE-----"));
    }

    #[test]
    fn verschiedene_certs_haben_verschiedene_fingerprints() {
        let config1 = generate_self_signed_cert("server-1").unwrap();
        let config2 = generate_self_signed_cert("server-2").unwrap();
        assert_ne!(
            config1.certificate_fingerprint,
            config2.certificate_fingerprint
        );
    }

    #[test]
    fn dtls_client_config_mit_fingerprint() {
        let config = DtlsClientConfig::with_fingerprint("AA:BB:CC".to_string());
        assert_eq!(config.expected_fingerprint, Some("AA:BB:CC".to_string()));
    }

    #[test]
    fn dtls_client_config_ohne_fingerprint() {
        let config = DtlsClientConfig::new();
        assert!(config.expected_fingerprint.is_none());
    }
}
