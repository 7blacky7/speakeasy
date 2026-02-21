//! Fehlertypen fuer das Plugin-System

use thiserror::Error;

/// Alle moeglichen Fehler im Plugin-System
#[derive(Debug, Error)]
pub enum PluginError {
    // --- Manifest ---
    #[error("Manifest-Fehler: {0}")]
    Manifest(String),

    #[error("Manifest nicht gefunden: {0}")]
    ManifestNichtGefunden(String),

    #[error("Ungueltige Plugin-Version: {0}")]
    UngueltigeVersion(String),

    // --- WASM Laufzeit ---
    #[error("WASM Ladefehler: {0}")]
    WasmLaden(String),

    #[error("WASM Kompilierungsfehler: {0}")]
    WasmKompilierung(String),

    #[error("WASM Instanziierungsfehler: {0}")]
    WasmInstanziierung(String),

    #[error("WASM Ausfuehrungsfehler: {0}")]
    WasmAusfuehrung(String),

    #[error("WASM Funktion nicht gefunden: {0}")]
    FunktionNichtGefunden(String),

    // --- Sicherheit / Trust ---
    #[error("Plugin-Signatur ungueltig")]
    SignaturUngueltig,

    #[error("Plugin nicht signiert – Signierung erforderlich")]
    NichtSigniert,

    #[error("Signierungsschluessel ungueltig: {0}")]
    SchluesselUngueltig(String),

    // --- Capabilities ---
    #[error("Fehlende Capability: {0}")]
    FehlendeFaehigkeit(String),

    #[error("Zugriff verweigert – Capability '{0}' nicht aktiviert")]
    ZugriffVerweigert(String),

    // --- Lifecycle ---
    #[error("Plugin nicht gefunden: {0}")]
    NichtGefunden(String),

    #[error("Plugin bereits geladen: {0}")]
    BereitsGeladen(String),

    #[error("Plugin ist nicht aktiv")]
    NichtAktiv,

    #[error("Plugin Initialisierung fehlgeschlagen: {0}")]
    Initialisierung(String),

    // --- Registry ---
    #[error("Registry-Fehler: {0}")]
    Registry(String),

    // --- IO ---
    #[error("IO-Fehler: {0}")]
    Io(#[from] std::io::Error),

    // --- Intern ---
    #[error("Interner Plugin-Fehler: {0}")]
    Intern(String),

    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
}

/// Result-Alias fuer das Plugin-System
pub type Result<T> = std::result::Result<T, PluginError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fehler_anzeige_manifest() {
        let e = PluginError::Manifest("Pflichtfeld fehlt: name".into());
        assert_eq!(e.to_string(), "Manifest-Fehler: Pflichtfeld fehlt: name");
    }

    #[test]
    fn fehler_anzeige_signatur() {
        let e = PluginError::SignaturUngueltig;
        assert_eq!(e.to_string(), "Plugin-Signatur ungueltig");
    }

    #[test]
    fn fehler_anzeige_capability() {
        let e = PluginError::ZugriffVerweigert("network".into());
        assert!(e.to_string().contains("network"));
    }

    #[test]
    fn io_fehler_konvertierung() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "Datei fehlt");
        let plugin_err: PluginError = io_err.into();
        assert!(plugin_err.to_string().contains("IO-Fehler"));
    }
}
