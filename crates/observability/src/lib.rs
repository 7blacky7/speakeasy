//! # speakeasy-observability
//!
//! Observability-Crate fuer Speakeasy:
//! - Prometheus-kompatible Metriken (`/metrics`)
//! - Health-Check-Endpunkt (`/health`)
//! - Structured JSON Logging via tracing-subscriber
//! - Request-Timing Middleware

pub mod health;
pub mod logging;
pub mod metrics;
pub mod middleware;

pub use health::{HealthResponse, HealthStatus, health_router};
pub use logging::logging_initialisieren;
pub use metrics::{SpeakeasyMetrics, metrics_router};
pub use middleware::request_timing_layer;

use anyhow::Result;
use std::net::SocketAddr;

/// Startet den Observability-HTTP-Server (Metriken + Health)
///
/// Endpunkte:
/// - `GET /metrics` – Prometheus scrape format
/// - `GET /health`  – Health-Check JSON
pub async fn observability_server_starten(bind_addr: SocketAddr) -> Result<()> {
    use axum::Router;

    let app = Router::new()
        .merge(metrics_router())
        .merge(health_router());

    let listener = tokio::net::TcpListener::bind(bind_addr).await?;
    tracing::info!(addr = %bind_addr, "Observability-Server gestartet");

    axum::serve(listener, app).await?;
    Ok(())
}
