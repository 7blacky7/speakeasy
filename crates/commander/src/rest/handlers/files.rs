//! REST-Handler fuer Datei-Endpunkte

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json, Response},
};
use serde_json::json;
use uuid::Uuid;

use crate::commands::types::Command;
use crate::rest::{session_aus_headers, CommanderState};

pub async fn list_files(
    State(state): State<CommanderState>,
    Path(kanal_id): Path<Uuid>,
    headers: HeaderMap,
) -> Response {
    let session = match session_aus_headers(&headers, &state) {
        Ok(s) => s,
        Err(r) => return r,
    };
    match state
        .ausfuehren(Command::DateiListe { kanal_id }, session)
        .await
    {
        Ok(resp) => (StatusCode::OK, Json(serde_json::to_value(resp).unwrap())).into_response(),
        Err(e) => (
            StatusCode::from_u16(e.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn delete_file(
    State(state): State<CommanderState>,
    Path(datei_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let session = match session_aus_headers(&headers, &state) {
        Ok(s) => s,
        Err(r) => return r,
    };
    match state
        .ausfuehren(Command::DateiLoeschen { datei_id }, session)
        .await
    {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (
            StatusCode::from_u16(e.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}
