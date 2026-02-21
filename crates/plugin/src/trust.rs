//! Signierung und Verifikation von WASM-Plugins via Ed25519
//!
//! Trust-Level:
//! - NichtSigniert: Warnung beim Laden
//! - Signiert: OK, manuelle Bestaetigung
//! - Vertrauenswuerdig: Auto-Load erlaubt

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use sha2::{Digest, Sha256};

use crate::error::{PluginError, Result};
use crate::types::TrustLevel;

/// Berechnet SHA-256 Hash des WASM-Bytecodes
pub fn wasm_hash(wasm_bytes: &[u8]) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(wasm_bytes);
    hasher.finalize().to_vec()
}

/// Signiert WASM-Bytecode mit einem privaten Ed25519-Schluessel
pub fn plugin_signieren(wasm_bytes: &[u8], signing_key: &SigningKey) -> Vec<u8> {
    let hash = wasm_hash(wasm_bytes);
    let sig: Signature = signing_key.sign(&hash);
    sig.to_bytes().to_vec()
}

/// Verifiziert die Signatur eines WASM-Plugins
pub fn plugin_verifizieren(
    wasm_bytes: &[u8],
    signatur_bytes: &[u8],
    verifying_key: &VerifyingKey,
) -> Result<bool> {
    let hash = wasm_hash(wasm_bytes);

    let sig_array: [u8; 64] = signatur_bytes
        .try_into()
        .map_err(|_| PluginError::SchluesselUngueltig("Signatur hat falsche Laenge".into()))?;

    let signature = Signature::from_bytes(&sig_array);

    Ok(verifying_key.verify(&hash, &signature).is_ok())
}

/// Bestimmt den Trust-Level eines Plugins
pub fn trust_level_bestimmen(
    wasm_bytes: &[u8],
    signatur: Option<&[u8]>,
    vertrauenswuerdige_keys: &[VerifyingKey],
) -> TrustLevel {
    let Some(sig_bytes) = signatur else {
        return TrustLevel::NichtSigniert;
    };

    // Gegen alle vertrauenswuerdigen Keys pruefen
    for key in vertrauenswuerdige_keys {
        if plugin_verifizieren(wasm_bytes, sig_bytes, key).unwrap_or(false) {
            return TrustLevel::Vertrauenswuerdig;
        }
    }

    // Signiert aber nicht von bekanntem Key
    TrustLevel::Signiert
}

/// Generiert ein neues Ed25519-Schluesselpaar (fuer Tests und Setup)
pub fn schluesselpaar_generieren() -> (SigningKey, VerifyingKey) {
    use ed25519_dalek::SigningKey;
    let mut csprng = rand_core::OsRng;
    // ed25519-dalek 2.x: SigningKey via from_bytes mit zufaelligem Schluessel
    let mut secret = [0u8; 32];
    use rand_core::RngCore;
    csprng.fill_bytes(&mut secret);
    let signing_key = SigningKey::from_bytes(&secret);
    let verifying_key = signing_key.verifying_key();
    (signing_key, verifying_key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_deterministisch() {
        let data = b"test wasm bytes";
        let h1 = wasm_hash(data);
        let h2 = wasm_hash(data);
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 32); // SHA-256 = 32 Bytes
    }

    #[test]
    fn hash_unterschiedlich_bei_verschiedenen_daten() {
        let h1 = wasm_hash(b"wasm-a");
        let h2 = wasm_hash(b"wasm-b");
        assert_ne!(h1, h2);
    }

    #[test]
    fn signieren_und_verifizieren_ok() {
        let (signing_key, verifying_key) = schluesselpaar_generieren();
        let wasm = b"(module)";
        let sig = plugin_signieren(wasm, &signing_key);
        let ok = plugin_verifizieren(wasm, &sig, &verifying_key).unwrap();
        assert!(ok, "Gueltige Signatur muss verifiziert werden");
    }

    #[test]
    fn verifizierung_falscher_key() {
        let (signing_key, _) = schluesselpaar_generieren();
        let (_, fremder_key) = schluesselpaar_generieren();
        let wasm = b"(module)";
        let sig = plugin_signieren(wasm, &signing_key);
        let ok = plugin_verifizieren(wasm, &sig, &fremder_key).unwrap();
        assert!(!ok, "Fremder Key darf nicht verifizieren");
    }

    #[test]
    fn verifizierung_manipulierter_wasm() {
        let (signing_key, verifying_key) = schluesselpaar_generieren();
        let wasm_original = b"(module)";
        let sig = plugin_signieren(wasm_original, &signing_key);
        // WASM wurde nach dem Signieren veraendert
        let wasm_manipuliert = b"(module ;; boese)";
        let ok = plugin_verifizieren(wasm_manipuliert, &sig, &verifying_key).unwrap();
        assert!(!ok, "Manipuliertes WASM darf nicht verifiziert werden");
    }

    #[test]
    fn trust_level_nicht_signiert() {
        let level = trust_level_bestimmen(b"wasm", None, &[]);
        assert_eq!(level, TrustLevel::NichtSigniert);
    }

    #[test]
    fn trust_level_signiert_unbekannter_key() {
        let (signing_key, _) = schluesselpaar_generieren();
        let wasm = b"(module)";
        let sig = plugin_signieren(wasm, &signing_key);
        // Leere vertrauenswuerdige Keys-Liste
        let level = trust_level_bestimmen(wasm, Some(&sig), &[]);
        assert_eq!(level, TrustLevel::Signiert);
    }

    #[test]
    fn trust_level_vertrauenswuerdig() {
        let (signing_key, verifying_key) = schluesselpaar_generieren();
        let wasm = b"(module)";
        let sig = plugin_signieren(wasm, &signing_key);
        let level = trust_level_bestimmen(wasm, Some(&sig), &[verifying_key]);
        assert_eq!(level, TrustLevel::Vertrauenswuerdig);
    }

    #[test]
    fn ungueltige_signatur_laenge() {
        let (_, verifying_key) = schluesselpaar_generieren();
        let err = plugin_verifizieren(b"wasm", b"zu_kurz", &verifying_key).unwrap_err();
        assert!(matches!(err, PluginError::SchluesselUngueltig(_)));
    }
}
