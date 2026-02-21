//! REST-Interface fuer den Speakeasy Commander

pub mod handlers;
pub mod middleware;
pub mod routes;
pub mod server;

use std::{future::Future, pin::Pin, sync::Arc};

use axum::{
    http::{StatusCode},
    response::{IntoResponse, Json, Response},
};
use serde_json::json;

use crate::auth::CommanderSession;
use crate::commands::types::{Command, Response as CmdResponse};
use crate::error::{CommanderError, CommanderResult};

/// Typ-Alias fuer eine geboxte Send-Future
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Funktor-Typ: empfaengt Command + Session, gibt CmdResponse zurueck
pub type ExecutorFn = Arc<
    dyn Fn(Command, CommanderSession) -> BoxFuture<'static, CommanderResult<CmdResponse>>
        + Send
        + Sync,
>;

/// Funktor-Typ: validiert Token synchron
pub type TokenValidatorFn =
    Arc<dyn Fn(&str) -> Result<CommanderSession, CommanderError> + Send + Sync>;

/// Axum-State fuer den Commander-REST-Server
#[derive(Clone)]
pub struct CommanderState {
    pub executor: ExecutorFn,
    pub token_validator: TokenValidatorFn,
}

impl CommanderState {
    pub fn neu(executor: ExecutorFn, token_validator: TokenValidatorFn) -> Self {
        Self { executor, token_validator }
    }

    /// Fuehrt einen Befehl aus
    pub fn ausfuehren(
        &self,
        cmd: Command,
        session: CommanderSession,
    ) -> BoxFuture<'static, CommanderResult<CmdResponse>> {
        (self.executor)(cmd, session)
    }
}

/// Extrahiert Session aus Axum-Request-Headern
pub fn session_aus_headers(
    headers: &axum::http::HeaderMap,
    state: &CommanderState,
) -> Result<CommanderSession, Response> {
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(json!({ "error": { "code": 401, "message": "Authorization-Header fehlt" } })),
            )
                .into_response()
        })?;

    (state.token_validator)(token).map_err(|_| {
        (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": { "code": 401, "message": "Ungueltiger oder abgelaufener Token" } })),
        )
            .into_response()
    })
}

// Fuer AppStateT-Kompatibilitaet (Typ-Alias fuer Abwaertskompatibilitaet)
pub use CommanderState as AppState;

pub use server::RestServer;
