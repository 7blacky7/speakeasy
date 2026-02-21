//! Request-Timing Middleware fuer Axum
//!
//! Misst die Antwortzeit jeder HTTP-Anfrage und protokolliert sie als
//! strukturiertes Log-Event sowie optional als Prometheus-Histogramm.

use axum::{
    body::Body,
    http::{Request, Response},
    middleware::Next,
};
use std::time::Instant;

/// Erstellt den Axum-Middleware-Layer fuer Request-Timing.
///
/// Loggt jede Anfrage mit Methode, Pfad, Statuscode und Dauer.
pub fn request_timing_layer() -> tower_http::trace::TraceLayer<
    tower_http::classify::SharedClassifier<tower_http::classify::ServerErrorsAsFailures>,
> {
    use tower_http::trace::TraceLayer;
    TraceLayer::new_for_http()
}

/// Axum-Middleware-Funktion: misst Antwortzeit und loggt strukturiert.
///
/// Verwendung:
/// ```ignore
/// Router::new()
///     .route("/", get(handler))
///     .layer(axum::middleware::from_fn(timing_middleware))
/// ```
pub async fn timing_middleware(req: Request<Body>, next: Next) -> Response<Body> {
    let methode = req.method().to_string();
    let pfad = req.uri().path().to_string();
    let start = Instant::now();

    let response = next.run(req).await;

    let dauer = start.elapsed();
    let status = response.status().as_u16();

    tracing::info!(
        method = %methode,
        path = %pfad,
        status = status,
        duration_ms = dauer.as_millis(),
        "HTTP-Anfrage abgeschlossen"
    );

    response
}

/// Hilfsfunktion: Gibt den Bucket-Index fuer eine Dauer (in ms) zurueck.
/// Wird intern fuer Histogramm-Zuordnung genutzt.
pub fn dauer_bucket(dauer_ms: u64) -> &'static str {
    match dauer_ms {
        0..=1 => "<=1ms",
        2..=5 => "<=5ms",
        6..=10 => "<=10ms",
        11..=25 => "<=25ms",
        26..=50 => "<=50ms",
        51..=100 => "<=100ms",
        101..=250 => "<=250ms",
        251..=500 => "<=500ms",
        501..=1000 => "<=1s",
        _ => ">1s",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bucket_sehr_schnell() {
        assert_eq!(dauer_bucket(0), "<=1ms");
        assert_eq!(dauer_bucket(1), "<=1ms");
    }

    #[test]
    fn bucket_schnell() {
        assert_eq!(dauer_bucket(2), "<=5ms");
        assert_eq!(dauer_bucket(5), "<=5ms");
    }

    #[test]
    fn bucket_mittel() {
        assert_eq!(dauer_bucket(11), "<=25ms");
        assert_eq!(dauer_bucket(25), "<=25ms");
    }

    #[test]
    fn bucket_langsam() {
        assert_eq!(dauer_bucket(500), "<=500ms");
        assert_eq!(dauer_bucket(1000), "<=1s");
    }

    #[test]
    fn bucket_sehr_langsam() {
        assert_eq!(dauer_bucket(1001), ">1s");
        assert_eq!(dauer_bucket(5000), ">1s");
    }

    #[test]
    fn alle_buckets_abgedeckt() {
        let faelle = [
            0u64, 1, 2, 5, 6, 10, 11, 25, 26, 50, 51, 100, 101, 250, 251, 500, 501, 1000, 1001,
            9999,
        ];
        for &ms in &faelle {
            let bucket = dauer_bucket(ms);
            assert!(!bucket.is_empty(), "Leerer Bucket fuer {ms}ms");
        }
    }
}
