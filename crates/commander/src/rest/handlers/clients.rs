//! REST-Handler fuer Client-Endpunkte

use axum::{extract::{Path, State}, http::{HeaderMap, StatusCode}, response::{IntoResponse, Json, Response}};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::commands::types::Command;
use crate::rest::{session_aus_headers, CommanderState};

pub async fn list_clients(State(state): State<CommanderState>, headers: HeaderMap) -> Response {
    let session = match session_aus_headers(&headers, &state) { Ok(s) => s, Err(r) => return r };
    match state.ausfuehren(Command::ClientListe, session).await {
        Ok(resp) => (StatusCode::OK, Json(serde_json::to_value(resp).unwrap())).into_response(),
        Err(e) => (StatusCode::from_u16(e.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR), Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct KickBody { pub grund: Option<String> }

pub async fn kick_client(State(state): State<CommanderState>, Path(id): Path<Uuid>, headers: HeaderMap, Json(body): Json<KickBody>) -> Response {
    let session = match session_aus_headers(&headers, &state) { Ok(s) => s, Err(r) => return r };
    match state.ausfuehren(Command::ClientKicken { client_id: id, grund: body.grund }, session).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::from_u16(e.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR), Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct BanBody { pub grund: Option<String>, pub dauer_secs: Option<u64>, pub ip_bannen: Option<bool> }

pub async fn ban_client(State(state): State<CommanderState>, Path(id): Path<Uuid>, headers: HeaderMap, Json(body): Json<BanBody>) -> Response {
    let session = match session_aus_headers(&headers, &state) { Ok(s) => s, Err(r) => return r };
    match state.ausfuehren(Command::ClientBannen { client_id: id, dauer_secs: body.dauer_secs, grund: body.grund, ip_bannen: body.ip_bannen.unwrap_or(false) }, session).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::from_u16(e.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR), Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct MoveBody { pub kanal_id: Uuid }

pub async fn move_client(State(state): State<CommanderState>, Path(id): Path<Uuid>, headers: HeaderMap, Json(body): Json<MoveBody>) -> Response {
    let session = match session_aus_headers(&headers, &state) { Ok(s) => s, Err(r) => return r };
    match state.ausfuehren(Command::ClientVerschieben { client_id: id, kanal_id: body.kanal_id }, session).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::from_u16(e.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR), Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct PokeBody { pub nachricht: String }

pub async fn poke_client(State(state): State<CommanderState>, Path(id): Path<Uuid>, headers: HeaderMap, Json(body): Json<PokeBody>) -> Response {
    let session = match session_aus_headers(&headers, &state) { Ok(s) => s, Err(r) => return r };
    match state.ausfuehren(Command::ClientPoken { client_id: id, nachricht: body.nachricht }, session).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::from_u16(e.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR), Json(json!({ "error": e.to_string() }))).into_response(),
    }
}
