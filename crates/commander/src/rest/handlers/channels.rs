//! REST-Handler fuer Kanal-Endpunkte

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json, Response},
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::commands::types::Command;
use crate::rest::{session_aus_headers, CommanderState};

pub async fn list_channels(State(state): State<CommanderState>, headers: HeaderMap) -> Response {
    let session = match session_aus_headers(&headers, &state) { Ok(s) => s, Err(r) => return r };
    match state.ausfuehren(Command::KanalListe, session).await {
        Ok(resp) => (StatusCode::OK, Json(serde_json::to_value(resp).unwrap())).into_response(),
        Err(e) => (StatusCode::from_u16(e.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR), Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct KanalErstellenBody {
    pub name: String,
    pub parent_id: Option<Uuid>,
    pub thema: Option<String>,
    pub passwort: Option<String>,
    pub max_clients: Option<i64>,
    pub sort_order: Option<i64>,
    pub permanent: Option<bool>,
}

pub async fn create_channel(State(state): State<CommanderState>, headers: HeaderMap, Json(body): Json<KanalErstellenBody>) -> Response {
    let session = match session_aus_headers(&headers, &state) { Ok(s) => s, Err(r) => return r };
    let cmd = Command::KanalErstellen {
        name: body.name, parent_id: body.parent_id, thema: body.thema,
        passwort: body.passwort, max_clients: body.max_clients.unwrap_or(0),
        sort_order: body.sort_order.unwrap_or(0), permanent: body.permanent.unwrap_or(false),
    };
    match state.ausfuehren(cmd, session).await {
        Ok(resp) => (StatusCode::CREATED, Json(serde_json::to_value(resp).unwrap())).into_response(),
        Err(e) => (StatusCode::from_u16(e.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR), Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct KanalBearbeitenBody {
    pub name: Option<String>,
    pub thema: Option<String>,
    pub max_clients: Option<i64>,
    pub sort_order: Option<i64>,
}

pub async fn update_channel(State(state): State<CommanderState>, Path(id): Path<Uuid>, headers: HeaderMap, Json(body): Json<KanalBearbeitenBody>) -> Response {
    let session = match session_aus_headers(&headers, &state) { Ok(s) => s, Err(r) => return r };
    let cmd = Command::KanalBearbeiten { id, name: body.name, thema: body.thema.map(Some), max_clients: body.max_clients, sort_order: body.sort_order };
    match state.ausfuehren(cmd, session).await {
        Ok(resp) => (StatusCode::OK, Json(serde_json::to_value(resp).unwrap())).into_response(),
        Err(e) => (StatusCode::from_u16(e.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR), Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

pub async fn delete_channel(State(state): State<CommanderState>, Path(id): Path<Uuid>, headers: HeaderMap) -> Response {
    let session = match session_aus_headers(&headers, &state) { Ok(s) => s, Err(r) => return r };
    match state.ausfuehren(Command::KanalLoeschen { id }, session).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::from_u16(e.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR), Json(json!({ "error": e.to_string() }))).into_response(),
    }
}
