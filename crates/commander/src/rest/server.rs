//! Axum HTTP-Server fuer den Commander

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::rate_limit::RateLimiter;
use crate::rest::{routes::v1_router, CommanderState};

/// REST-Server-Konfiguration
#[derive(Debug, Clone)]
pub struct RestServerKonfig {
    pub bind_addr: SocketAddr,
    pub cors_origins: Vec<String>,
}

impl Default for RestServerKonfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:9300".parse().unwrap(),
            cors_origins: vec![],
        }
    }
}

/// Axum HTTP-Server fuer den Commander
pub struct RestServer {
    konfig: RestServerKonfig,
}

impl RestServer {
    pub fn neu(konfig: RestServerKonfig) -> Self {
        Self { konfig }
    }

    /// Startet den REST-Server mit dem gegebenen State
    pub async fn starten(
        self,
        state: CommanderState,
        _rate_limiter: Arc<RateLimiter>,
    ) -> Result<()> {
        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);

        let app = v1_router()
            .layer(TraceLayer::new_for_http())
            .layer(cors)
            .with_state(state);

        let listener = tokio::net::TcpListener::bind(self.konfig.bind_addr).await?;
        tracing::info!(addr = %self.konfig.bind_addr, "REST-Commander-Server gestartet");

        axum::serve(listener, app).await?;
        Ok(())
    }
}

/// GET /health – Health-Check-Endpunkt
pub async fn health() -> impl IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({ "status": "ok" })))
}
