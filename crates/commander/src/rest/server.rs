//! Axum HTTP-Server fuer den Commander

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use axum::body::Body;
use axum::extract::State;
use axum::http::Request;
use axum::http::{HeaderValue, Method, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Json, Response};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::rate_limit::RateLimiter;
use crate::rest::{routes::v1_router, CommanderState};

/// REST-Server-Konfiguration
#[derive(Debug, Clone)]
pub struct RestServerKonfig {
    pub bind_addr: SocketAddr,
    /// Erlaubte CORS-Origins. Leer = alle Origins erlaubt (nur fuer Entwicklung).
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

/// Axum-State der den RateLimiter enthaelt (fuer die Middleware)
#[derive(Clone)]
struct RateLimitState {
    limiter: Arc<RateLimiter>,
}

/// Axum-Middleware: Rate Limiting per IP
async fn rate_limit_middleware(
    State(rls): State<RateLimitState>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let ip = req
        .headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .unwrap_or("unknown")
        .trim()
        .to_string();

    match rls.limiter.pruefe_ip(&ip) {
        Ok(()) => next.run(req).await,
        Err(retry_after) => (
            StatusCode::TOO_MANY_REQUESTS,
            Json(serde_json::json!({
                "error": {
                    "code": 429,
                    "message": "Rate-Limit ueberschritten",
                    "retry_after_secs": retry_after
                }
            })),
        )
            .into_response(),
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

    /// Startet den REST-Server mit dem gegebenen State und Rate Limiter
    pub async fn starten(
        self,
        state: CommanderState,
        rate_limiter: Arc<RateLimiter>,
    ) -> Result<()> {
        // CORS konfigurieren: entweder spezifische Origins oder Any
        let cors = if self.konfig.cors_origins.is_empty() {
            CorsLayer::permissive()
        } else {
            let origins: Vec<HeaderValue> = self
                .konfig
                .cors_origins
                .iter()
                .filter_map(|o| o.parse().ok())
                .collect();
            CorsLayer::new()
                .allow_origin(origins)
                .allow_methods([
                    Method::GET,
                    Method::POST,
                    Method::PUT,
                    Method::DELETE,
                    Method::OPTIONS,
                ])
                .allow_headers(tower_http::cors::Any)
        };

        let rls = RateLimitState {
            limiter: rate_limiter,
        };

        let app = v1_router()
            // Rate Limiter als innersten Layer (laeuft vor den Handlern)
            .layer(middleware::from_fn_with_state(rls, rate_limit_middleware))
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
