//! REST-Handler fuer Log-Endpunkte

use axum::{extract::{Query, State}, http::{HeaderMap, StatusCode}, response::{IntoResponse, Json, Response}};
use serde::Deserialize;
use serde_json::json;

use crate::commands::types::Command;
use crate::rest::{session_aus_headers, CommanderState};

#[derive(Debug, Deserialize)]
pub struct LogQuery { pub limit: Option<u32>, pub offset: Option<u32>, pub aktion: Option<String> }

pub async fn get_logs(State(state): State<CommanderState>, Query(params): Query<LogQuery>, headers: HeaderMap) -> Response {
    let session = match session_aus_headers(&headers, &state) { Ok(s) => s, Err(r) => return r };
    match state.ausfuehren(Command::LogAbfragen { limit: params.limit.unwrap_or(50).min(1000), offset: params.offset.unwrap_or(0), aktion_filter: params.aktion }, session).await {
        Ok(resp) => (StatusCode::OK, Json(serde_json::to_value(resp).unwrap())).into_response(),
        Err(e) => (StatusCode::from_u16(e.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR), Json(json!({ "error": e.to_string() }))).into_response(),
    }
}
