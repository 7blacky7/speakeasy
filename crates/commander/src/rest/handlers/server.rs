//! REST-Handler fuer Server-Endpunkte

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json, Response},
};
use serde::Deserialize;
use serde_json::json;

use crate::commands::types::Command;
use crate::rest::{session_aus_headers, CommanderState};

pub async fn get_server(State(state): State<CommanderState>, headers: HeaderMap) -> Response {
    let session = match session_aus_headers(&headers, &state) {
        Ok(s) => s,
        Err(r) => return r,
    };
    match state.ausfuehren(Command::ServerInfo, session).await {
        Ok(resp) => (StatusCode::OK, Json(serde_json::to_value(resp).unwrap())).into_response(),
        Err(e) => (
            StatusCode::from_u16(e.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct ServerBearbeitenBody {
    pub name: Option<String>,
    pub willkommensnachricht: Option<String>,
    pub max_clients: Option<u32>,
    pub host_nachricht: Option<String>,
}

pub async fn put_server(
    State(state): State<CommanderState>,
    headers: HeaderMap,
    Json(body): Json<ServerBearbeitenBody>,
) -> Response {
    let session = match session_aus_headers(&headers, &state) {
        Ok(s) => s,
        Err(r) => return r,
    };
    let cmd = Command::ServerEdit {
        name: body.name,
        willkommensnachricht: body.willkommensnachricht,
        max_clients: body.max_clients,
        host_nachricht: body.host_nachricht,
    };
    match state.ausfuehren(cmd, session).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (
            StatusCode::from_u16(e.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct ServerStoppenBody {
    pub grund: Option<String>,
}

pub async fn post_server_stop(
    State(state): State<CommanderState>,
    headers: HeaderMap,
    Json(body): Json<ServerStoppenBody>,
) -> Response {
    let session = match session_aus_headers(&headers, &state) {
        Ok(s) => s,
        Err(r) => return r,
    };
    match state
        .ausfuehren(Command::ServerStop { grund: body.grund }, session)
        .await
    {
        Ok(_) => StatusCode::ACCEPTED.into_response(),
        Err(e) => (
            StatusCode::from_u16(e.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}
