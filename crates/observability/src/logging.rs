//! Structured Logging Setup via tracing-subscriber
//!
//! Konfigurierbar per Umgebungsvariable:
//! - `SE_LOG_LEVEL`: Log-Level (trace/debug/info/warn/error), Standard: info
//! - `SE_LOG_FORMAT`: Format (text/json), Standard: text
//!
//! Request-IDs werden als Tracing-Span-Felder propagiert.

use tracing_subscriber::{EnvFilter, fmt};

/// Initialisiert das Logging-System.
///
/// Liest `SE_LOG_LEVEL` und `SE_LOG_FORMAT` aus der Umgebung.
/// Faellt auf `info` / `text` zurueck falls nicht gesetzt.
pub fn logging_initialisieren(level: &str, format: &str) {
    let filter = EnvFilter::try_from_env("SE_LOG_LEVEL")
        .or_else(|_| EnvFilter::try_new(level))
        .unwrap_or_else(|_| EnvFilter::new("info"));

    let format_env = std::env::var("SE_LOG_FORMAT")
        .unwrap_or_else(|_| format.to_string());

    match format_env.as_str() {
        "json" => {
            fmt()
                .json()
                .with_env_filter(filter)
                .with_target(true)
                .with_thread_ids(true)
                .with_current_span(true)
                .init();
        }
        _ => {
            fmt()
                .with_env_filter(filter)
                .with_target(true)
                .init();
        }
    }
}

/// Gibt den konfigurierten Log-Level aus der Umgebung zurueck.
/// Fallback: "info"
pub fn log_level_aus_env() -> String {
    std::env::var("SE_LOG_LEVEL").unwrap_or_else(|_| "info".to_string())
}

/// Gibt das konfigurierte Log-Format aus der Umgebung zurueck.
/// Fallback: "text"
pub fn log_format_aus_env() -> String {
    std::env::var("SE_LOG_FORMAT").unwrap_or_else(|_| "text".to_string())
}

/// Validiert ob ein Log-Level-String gueltig ist.
pub fn log_level_gueltig(level: &str) -> bool {
    matches!(level, "trace" | "debug" | "info" | "warn" | "error")
}

/// Validiert ob ein Log-Format-String gueltig ist.
pub fn log_format_gueltig(format: &str) -> bool {
    matches!(format, "text" | "json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_level_gueltige_werte() {
        assert!(log_level_gueltig("trace"));
        assert!(log_level_gueltig("debug"));
        assert!(log_level_gueltig("info"));
        assert!(log_level_gueltig("warn"));
        assert!(log_level_gueltig("error"));
    }

    #[test]
    fn log_level_ungueltige_werte() {
        assert!(!log_level_gueltig("verbose"));
        assert!(!log_level_gueltig("INFO")); // Gross-/Kleinschreibung
        assert!(!log_level_gueltig(""));
        assert!(!log_level_gueltig("critical"));
    }

    #[test]
    fn log_format_gueltige_werte() {
        assert!(log_format_gueltig("text"));
        assert!(log_format_gueltig("json"));
    }

    #[test]
    fn log_format_ungueltige_werte() {
        assert!(!log_format_gueltig("xml"));
        assert!(!log_format_gueltig("JSON")); // Gross-/Kleinschreibung
        assert!(!log_format_gueltig(""));
    }

    #[test]
    fn log_level_aus_env_fallback() {
        // Ohne gesetzte Umgebungsvariable -> Fallback "info"
        std::env::remove_var("SE_LOG_LEVEL");
        assert_eq!(log_level_aus_env(), "info");
    }

    #[test]
    fn log_format_aus_env_fallback() {
        // Ohne gesetzte Umgebungsvariable -> Fallback "text"
        std::env::remove_var("SE_LOG_FORMAT");
        assert_eq!(log_format_aus_env(), "text");
    }

    #[test]
    fn log_level_aus_env_gesetzt() {
        std::env::set_var("SE_LOG_LEVEL", "debug");
        assert_eq!(log_level_aus_env(), "debug");
        std::env::remove_var("SE_LOG_LEVEL");
    }

    #[test]
    fn log_format_aus_env_json() {
        std::env::set_var("SE_LOG_FORMAT", "json");
        assert_eq!(log_format_aus_env(), "json");
        std::env::remove_var("SE_LOG_FORMAT");
    }
}
