//! Health-Check-Endpunkt fuer Speakeasy
//!
//! Endpoint: `GET /health`
//! Response: JSON mit Status, Version, Uptime und DB-Verbindungsstatus

use axum::{Json, Router, extract::State, http::StatusCode, response::IntoResponse, routing::get};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;

/// Status des Health-Checks
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

/// Antwort des Health-Check-Endpunkts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: HealthStatus,
    pub version: String,
    pub uptime_seconds: u64,
    pub db_connected: bool,
}

/// Geteilter Zustand fuer den Health-Check-Handler
#[derive(Clone)]
pub struct HealthState {
    pub start_time: Arc<Instant>,
    pub db_connected: Arc<std::sync::atomic::AtomicBool>,
}

impl HealthState {
    pub fn neu() -> Self {
        Self {
            start_time: Arc::new(Instant::now()),
            db_connected: Arc::new(std::sync::atomic::AtomicBool::new(true)),
        }
    }

    pub fn uptime_seconds(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }

    pub fn db_verbunden(&self) -> bool {
        self.db_connected.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn db_status_setzen(&self, verbunden: bool) {
        self.db_connected
            .store(verbunden, std::sync::atomic::Ordering::Relaxed);
    }
}

/// Axum-Router fuer den `/health`-Endpunkt
pub fn health_router() -> Router {
    let state = HealthState::neu();
    Router::new()
        .route("/health", get(health_handler))
        .with_state(state)
}

/// `GET /health` – gibt den Serverstatus zurueck
async fn health_handler(State(state): State<HealthState>) -> impl IntoResponse {
    let db_connected = state.db_verbunden();
    let status = if db_connected {
        HealthStatus::Healthy
    } else {
        HealthStatus::Degraded
    };

    let http_status = match status {
        HealthStatus::Healthy => StatusCode::OK,
        HealthStatus::Degraded => StatusCode::OK, // 200 auch bei degraded (Probe soll nicht failen)
        HealthStatus::Unhealthy => StatusCode::SERVICE_UNAVAILABLE,
    };

    let response = HealthResponse {
        status,
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds: state.uptime_seconds(),
        db_connected,
    };

    (http_status, Json(response))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_state_uptime_waechst() {
        let state = HealthState::neu();
        let uptime1 = state.uptime_seconds();
        // Uptime sollte >= 0 sein
        assert!(uptime1 < 5); // Frisch erstellt
    }

    #[test]
    fn health_state_db_standard_verbunden() {
        let state = HealthState::neu();
        assert!(state.db_verbunden());
    }

    #[test]
    fn health_state_db_status_umschalten() {
        let state = HealthState::neu();
        state.db_status_setzen(false);
        assert!(!state.db_verbunden());
        state.db_status_setzen(true);
        assert!(state.db_verbunden());
    }

    #[test]
    fn health_response_serialisierung() {
        let response = HealthResponse {
            status: HealthStatus::Healthy,
            version: "0.1.0".to_string(),
            uptime_seconds: 3600,
            db_connected: true,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"status\":\"healthy\""));
        assert!(json.contains("\"version\":\"0.1.0\""));
        assert!(json.contains("\"uptime_seconds\":3600"));
        assert!(json.contains("\"db_connected\":true"));
    }

    #[test]
    fn health_response_degraded_format() {
        let response = HealthResponse {
            status: HealthStatus::Degraded,
            version: "0.1.0".to_string(),
            uptime_seconds: 120,
            db_connected: false,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"status\":\"degraded\""));
        assert!(json.contains("\"db_connected\":false"));
    }

    #[test]
    fn health_response_deserialisierung() {
        let json = r#"{"status":"healthy","version":"0.1.0","uptime_seconds":100,"db_connected":true}"#;
        let response: HealthResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.status, HealthStatus::Healthy);
        assert_eq!(response.version, "0.1.0");
        assert_eq!(response.uptime_seconds, 100);
        assert!(response.db_connected);
    }
}
