//! REST-Handler fuer Berechtigungs-Endpunkte

use axum::{extract::{Path, State}, http::{HeaderMap, StatusCode}, response::{IntoResponse, Json, Response}};
use serde::Deserialize;
use serde_json::json;

use crate::commands::types::{BerechtigungsWertInput, Command};
use crate::rest::{session_aus_headers, CommanderState};

pub async fn get_permissions(State(state): State<CommanderState>, Path(ziel): Path<String>, headers: HeaderMap) -> Response {
    let session = match session_aus_headers(&headers, &state) { Ok(s) => s, Err(r) => return r };
    match state.ausfuehren(Command::BerechtigungListe { ziel, scope: "server".into() }, session).await {
        Ok(resp) => (StatusCode::OK, Json(serde_json::to_value(resp).unwrap())).into_response(),
        Err(e) => (StatusCode::from_u16(e.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR), Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct SetPermissionBody { pub ziel: String, pub permission: String, pub wert: BerechtigungsWertInput, pub scope: Option<String> }

pub async fn set_permission(State(state): State<CommanderState>, headers: HeaderMap, Json(body): Json<SetPermissionBody>) -> Response {
    let session = match session_aus_headers(&headers, &state) { Ok(s) => s, Err(r) => return r };
    match state.ausfuehren(Command::BerechtigungSetzen { ziel: body.ziel, permission: body.permission, wert: body.wert, scope: body.scope.unwrap_or_else(|| "server".into()) }, session).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::from_u16(e.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR), Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct RemovePermissionBody { pub ziel: String, pub scope: Option<String> }

pub async fn remove_permission(State(state): State<CommanderState>, Path(permission): Path<String>, headers: HeaderMap, Json(body): Json<RemovePermissionBody>) -> Response {
    let session = match session_aus_headers(&headers, &state) { Ok(s) => s, Err(r) => return r };
    match state.ausfuehren(Command::BerechtigungEntfernen { ziel: body.ziel, permission, scope: body.scope.unwrap_or_else(|| "server".into()) }, session).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::from_u16(e.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR), Json(json!({ "error": e.to_string() }))).into_response(),
    }
}
