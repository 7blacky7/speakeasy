//! Axum-Middleware fuer Auth und Rate Limiting

use axum::{
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json, Response},
};
use serde_json::json;

use crate::auth::CommanderSession;

/// Extrahiert den Client-IP aus den Request-Headern
pub fn client_ip(headers: &HeaderMap) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Fehlerantwort fuer REST-API
pub fn fehler_antwort(status: StatusCode, nachricht: &str, code: u32) -> Response {
    (
        status,
        Json(json!({
            "error": {
                "code": code,
                "message": nachricht
            }
        })),
    )
        .into_response()
}

/// Extrahiert Bearer-Token aus Authorization-Header
pub fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
}

/// Wrapper-Typ fuer die authentifizierte Session (als Extension gespeichert)
#[derive(Clone)]
pub struct AuthSession(pub CommanderSession);

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn client_ip_aus_x_forwarded_for() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            HeaderValue::from_static("192.168.1.1, 10.0.0.1"),
        );
        assert_eq!(client_ip(&headers), "192.168.1.1");
    }

    #[test]
    fn client_ip_ohne_header() {
        let headers = HeaderMap::new();
        assert_eq!(client_ip(&headers), "unknown");
    }

    #[test]
    fn bearer_token_extrahieren() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            HeaderValue::from_static("Bearer mein_token_123"),
        );
        assert_eq!(bearer_token(&headers), Some("mein_token_123"));
    }

    #[test]
    fn bearer_token_fehlt() {
        let headers = HeaderMap::new();
        assert_eq!(bearer_token(&headers), None);
    }
}
