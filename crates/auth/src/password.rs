//! Passwort-Hashing mit Argon2id
//!
//! Stellt sichere Passwort-Hashfunktionen mit Argon2id bereit.
//! Argon2id ist der empfohlene Algorithmus gemaess OWASP-Richtlinien.

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2, Params, Version,
};

use crate::error::AuthError;

/// Argon2id-Parameter fuer sicheres Passwort-Hashing
///
/// Werte gemaess OWASP-Empfehlungen (Stand 2024):
/// - Speicher: 64 MiB
/// - Iterationen: 3
/// - Parallelismus: 1
fn argon2_instanz() -> Argon2<'static> {
    let params = Params::new(
        64 * 1024, // m_cost: 64 MiB
        3,         // t_cost: 3 Iterationen
        1,         // p_cost: 1 Thread
        None,      // output_len: Standard (32 Bytes)
    )
    .expect("Argon2-Parameter ungueltig");

    Argon2::new(argon2::Algorithm::Argon2id, Version::V0x13, params)
}

/// Hasht ein Passwort mit Argon2id und einem zufaelligen Salt
///
/// Gibt den PHC-String zurueck (inkl. Algorithmus, Parameter und Salt).
pub fn passwort_hashen(passwort: &str) -> Result<String, AuthError> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = argon2_instanz();

    argon2
        .hash_password(passwort.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|e| AuthError::PasswortHashing(e.to_string()))
}

/// Verifiziert ein Passwort gegen einen gespeicherten PHC-Hash
///
/// Gibt `true` zurueck wenn das Passwort korrekt ist.
pub fn passwort_verifizieren(passwort: &str, hash: &str) -> Result<bool, AuthError> {
    let parsed_hash = PasswordHash::new(hash)
        .map_err(|e| AuthError::PasswortHashing(format!("Ungültiger Hash-Format: {e}")))?;

    match argon2_instanz().verify_password(passwort.as_bytes(), &parsed_hash) {
        Ok(()) => Ok(true),
        Err(argon2::password_hash::Error::Password) => Ok(false),
        Err(e) => Err(AuthError::PasswortHashing(e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passwort_hashen_und_verifizieren() {
        let passwort = "sicheres_passwort_123!";
        let hash = passwort_hashen(passwort).expect("Hashing fehlgeschlagen");

        assert!(!hash.is_empty());
        assert!(
            hash.starts_with("$argon2id$"),
            "Hash muss mit $argon2id$ beginnen"
        );

        let korrekt = passwort_verifizieren(passwort, &hash).expect("Verifikation fehlgeschlagen");
        assert!(korrekt, "Passwort muss korrekt verifiziert werden");
    }

    #[test]
    fn falsches_passwort_wird_abgelehnt() {
        let passwort = "richtiges_passwort";
        let hash = passwort_hashen(passwort).expect("Hashing fehlgeschlagen");

        let korrekt =
            passwort_verifizieren("falsches_passwort", &hash).expect("Verifikation fehlgeschlagen");
        assert!(!korrekt, "Falsches Passwort muss abgelehnt werden");
    }

    #[test]
    fn gleiche_passwoerter_unterschiedliche_hashes() {
        let passwort = "gleiches_passwort";
        let hash1 = passwort_hashen(passwort).expect("Hashing 1 fehlgeschlagen");
        let hash2 = passwort_hashen(passwort).expect("Hashing 2 fehlgeschlagen");

        assert_ne!(
            hash1, hash2,
            "Gleiche Passwoerter muessen verschiedene Hashes erzeugen (Salt)"
        );
    }

    #[test]
    fn ungueltige_hash_format_gibt_fehler() {
        let ergebnis = passwort_verifizieren("passwort", "kein_gueltiger_hash");
        assert!(
            ergebnis.is_err(),
            "Ungueltig formatierter Hash muss Fehler zurueckgeben"
        );
    }
}
