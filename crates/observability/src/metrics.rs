//! Prometheus-kompatible Metriken fuer Speakeasy
//!
//! Registrierte Metriken:
//! - `speakeasy_connected_clients` – Gauge: Aktuell verbundene Clients
//! - `speakeasy_voice_channels_active` – Gauge: Aktive Voice-Kanaele
//! - `speakeasy_voice_packets_total` – Counter: Gesendete Voice-Pakete
//! - `speakeasy_voice_packet_loss_ratio` – Histogram: Paketverlust-Rate
//! - `speakeasy_voice_rtt_seconds` – Histogram: Round-Trip-Time
//! - `speakeasy_voice_jitter_seconds` – Histogram: Jitter
//! - `speakeasy_voice_bitrate_bps` – Gauge: Aktuelle Bitrate
//! - `speakeasy_cpu_usage_percent` – Gauge: CPU-Auslastung
//! - `speakeasy_memory_usage_bytes` – Gauge: Speicherverbrauch
//! - `speakeasy_http_requests_total` – Counter: HTTP-Anfragen (method, path, status)
//! - `speakeasy_http_request_duration_seconds` – Histogram: HTTP-Antwortzeit

use anyhow::Result;
use axum::{response::IntoResponse, routing::get, Router};
use prometheus::{
    Counter, Encoder, Gauge, Histogram, HistogramOpts, HistogramVec, IntCounterVec, Opts, Registry,
    TextEncoder,
};
use std::sync::Arc;

/// Alle Speakeasy-Prometheus-Metriken
#[derive(Clone)]
pub struct SpeakeasyMetrics {
    pub registry: Arc<Registry>,

    // Voice-Metriken
    pub connected_clients: Gauge,
    pub voice_channels_active: Gauge,
    pub voice_packets_total: Counter,
    pub voice_packet_loss_ratio: Histogram,
    pub voice_rtt_seconds: Histogram,
    pub voice_jitter_seconds: Histogram,
    pub voice_bitrate_bps: Gauge,

    // System-Metriken
    pub cpu_usage_percent: Gauge,
    pub memory_usage_bytes: Gauge,

    // HTTP-Metriken
    pub http_requests_total: IntCounterVec,
    pub http_request_duration_seconds: HistogramVec,
}

impl SpeakeasyMetrics {
    /// Erstellt und registriert alle Metriken in einer neuen Registry
    pub fn neu() -> Result<Self> {
        let registry = Registry::new();

        // --- Voice-Metriken ---
        let connected_clients = Gauge::with_opts(Opts::new(
            "speakeasy_connected_clients",
            "Anzahl aktuell verbundener Clients",
        ))?;
        registry.register(Box::new(connected_clients.clone()))?;

        let voice_channels_active = Gauge::with_opts(Opts::new(
            "speakeasy_voice_channels_active",
            "Anzahl aktiver Voice-Kanaele",
        ))?;
        registry.register(Box::new(voice_channels_active.clone()))?;

        let voice_packets_total = Counter::with_opts(Opts::new(
            "speakeasy_voice_packets_total",
            "Gesamtanzahl gesendeter Voice-Pakete",
        ))?;
        registry.register(Box::new(voice_packets_total.clone()))?;

        let voice_packet_loss_ratio = Histogram::with_opts(
            HistogramOpts::new(
                "speakeasy_voice_packet_loss_ratio",
                "Paketverlust-Rate (0.0 bis 1.0)",
            )
            .buckets(vec![0.0, 0.01, 0.02, 0.05, 0.1, 0.2, 0.5, 1.0]),
        )?;
        registry.register(Box::new(voice_packet_loss_ratio.clone()))?;

        let voice_rtt_seconds = Histogram::with_opts(
            HistogramOpts::new("speakeasy_voice_rtt_seconds", "Round-Trip-Time in Sekunden")
                .buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0]),
        )?;
        registry.register(Box::new(voice_rtt_seconds.clone()))?;

        let voice_jitter_seconds = Histogram::with_opts(
            HistogramOpts::new("speakeasy_voice_jitter_seconds", "Voice-Jitter in Sekunden")
                .buckets(vec![0.001, 0.002, 0.005, 0.01, 0.02, 0.05, 0.1]),
        )?;
        registry.register(Box::new(voice_jitter_seconds.clone()))?;

        let voice_bitrate_bps = Gauge::with_opts(Opts::new(
            "speakeasy_voice_bitrate_bps",
            "Aktuelle Voice-Bitrate in Bits pro Sekunde",
        ))?;
        registry.register(Box::new(voice_bitrate_bps.clone()))?;

        // --- System-Metriken ---
        let cpu_usage_percent = Gauge::with_opts(Opts::new(
            "speakeasy_cpu_usage_percent",
            "CPU-Auslastung in Prozent (0-100)",
        ))?;
        registry.register(Box::new(cpu_usage_percent.clone()))?;

        let memory_usage_bytes = Gauge::with_opts(Opts::new(
            "speakeasy_memory_usage_bytes",
            "Speicherverbrauch in Bytes",
        ))?;
        registry.register(Box::new(memory_usage_bytes.clone()))?;

        // --- HTTP-Metriken ---
        let http_requests_total = IntCounterVec::new(
            Opts::new(
                "speakeasy_http_requests_total",
                "Gesamtanzahl HTTP-Anfragen",
            ),
            &["method", "path", "status"],
        )?;
        registry.register(Box::new(http_requests_total.clone()))?;

        let http_request_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "speakeasy_http_request_duration_seconds",
                "HTTP-Antwortzeit in Sekunden",
            )
            .buckets(vec![
                0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5,
            ]),
            &["method", "path"],
        )?;
        registry.register(Box::new(http_request_duration_seconds.clone()))?;

        Ok(Self {
            registry: Arc::new(registry),
            connected_clients,
            voice_channels_active,
            voice_packets_total,
            voice_packet_loss_ratio,
            voice_rtt_seconds,
            voice_jitter_seconds,
            voice_bitrate_bps,
            cpu_usage_percent,
            memory_usage_bytes,
            http_requests_total,
            http_request_duration_seconds,
        })
    }

    /// Exportiert alle Metriken im Prometheus-Textformat
    pub fn exportieren(&self) -> Result<String> {
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer)?;
        Ok(String::from_utf8(buffer)?)
    }
}

/// Axum-Router fuer den `/metrics`-Endpunkt
pub fn metrics_router() -> Router {
    use std::sync::OnceLock;
    static METRIKEN: OnceLock<SpeakeasyMetrics> = OnceLock::new();
    let metriken = METRIKEN
        .get_or_init(|| SpeakeasyMetrics::neu().expect("Metriken-Initialisierung fehlgeschlagen"));

    Router::new()
        .route("/metrics", get(metrics_handler))
        .with_state(metriken.clone())
}

async fn metrics_handler(
    axum::extract::State(metriken): axum::extract::State<SpeakeasyMetrics>,
) -> impl IntoResponse {
    match metriken.exportieren() {
        Ok(text) => (
            axum::http::StatusCode::OK,
            [(
                axum::http::header::CONTENT_TYPE,
                "text/plain; version=0.0.4",
            )],
            text,
        )
            .into_response(),
        Err(err) => {
            tracing::error!("Metriken-Export fehlgeschlagen: {err}");
            axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metriken_erstellen_erfolgreich() {
        let metriken = SpeakeasyMetrics::neu().unwrap();
        // Registry muss Metriken enthalten
        assert!(!metriken.registry.gather().is_empty());
    }

    #[test]
    fn gauge_connected_clients_setzen() {
        let metriken = SpeakeasyMetrics::neu().unwrap();
        metriken.connected_clients.set(42.0);
        assert_eq!(metriken.connected_clients.get(), 42.0);
    }

    #[test]
    fn counter_voice_packets_inkrementieren() {
        let metriken = SpeakeasyMetrics::neu().unwrap();
        metriken.voice_packets_total.inc();
        metriken.voice_packets_total.inc_by(9.0);
        assert_eq!(metriken.voice_packets_total.get(), 10.0);
    }

    #[test]
    fn histogram_rtt_beobachten() {
        let metriken = SpeakeasyMetrics::neu().unwrap();
        metriken.voice_rtt_seconds.observe(0.025);
        metriken.voice_rtt_seconds.observe(0.050);
        // Kein Panic = Erfolg
    }

    #[test]
    fn http_counter_mit_labels() {
        let metriken = SpeakeasyMetrics::neu().unwrap();
        metriken
            .http_requests_total
            .with_label_values(&["GET", "/health", "200"])
            .inc();
        let wert = metriken
            .http_requests_total
            .with_label_values(&["GET", "/health", "200"])
            .get();
        assert_eq!(wert, 1);
    }

    #[test]
    fn metriken_export_prometheus_format() {
        let metriken = SpeakeasyMetrics::neu().unwrap();
        metriken.connected_clients.set(5.0);
        metriken.voice_packets_total.inc();

        let output = metriken.exportieren().unwrap();
        assert!(output.contains("speakeasy_connected_clients"));
        assert!(output.contains("speakeasy_voice_packets_total"));
        assert!(output.contains("# HELP"));
        assert!(output.contains("# TYPE"));
    }

    #[test]
    fn alle_metriken_in_registry_registriert() {
        let metriken = SpeakeasyMetrics::neu().unwrap();

        // Vec-Metriken (IntCounterVec, HistogramVec) erscheinen in gather() erst
        // nach dem ersten Label-Zugriff – daher einmal initialisieren.
        metriken
            .http_requests_total
            .with_label_values(&["GET", "/test", "200"])
            .inc();
        metriken
            .http_request_duration_seconds
            .with_label_values(&["GET", "/test"])
            .observe(0.01);

        let families = metriken.registry.gather();
        let namen: Vec<&str> = families.iter().map(|f| f.get_name()).collect();

        assert!(namen.contains(&"speakeasy_connected_clients"));
        assert!(namen.contains(&"speakeasy_voice_channels_active"));
        assert!(namen.contains(&"speakeasy_voice_packets_total"));
        assert!(namen.contains(&"speakeasy_voice_packet_loss_ratio"));
        assert!(namen.contains(&"speakeasy_voice_rtt_seconds"));
        assert!(namen.contains(&"speakeasy_voice_jitter_seconds"));
        assert!(namen.contains(&"speakeasy_voice_bitrate_bps"));
        assert!(namen.contains(&"speakeasy_cpu_usage_percent"));
        assert!(namen.contains(&"speakeasy_memory_usage_bytes"));
        assert!(namen.contains(&"speakeasy_http_requests_total"));
        assert!(namen.contains(&"speakeasy_http_request_duration_seconds"));
    }
}
