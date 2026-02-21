//! Route-Definitionen fuer die REST-API (/v1/...)

use axum::{
    routing::{delete, get, post, put},
    Router,
};

use crate::rest::{handlers, CommanderState};

/// Erstellt den vollstaendigen /v1/-Router
pub fn v1_router() -> Router<CommanderState> {
    Router::new()
        // Server
        .route("/v1/server", get(handlers::server::get_server))
        .route("/v1/server", put(handlers::server::put_server))
        .route("/v1/server/stop", post(handlers::server::post_server_stop))
        // Kanaele
        .route("/v1/channels", get(handlers::channels::list_channels))
        .route("/v1/channels", post(handlers::channels::create_channel))
        .route("/v1/channels/:id", put(handlers::channels::update_channel))
        .route(
            "/v1/channels/:id",
            delete(handlers::channels::delete_channel),
        )
        // Clients
        .route("/v1/clients", get(handlers::clients::list_clients))
        .route("/v1/clients/:id/kick", post(handlers::clients::kick_client))
        .route("/v1/clients/:id/ban", post(handlers::clients::ban_client))
        .route("/v1/clients/:id/move", post(handlers::clients::move_client))
        .route("/v1/clients/:id/poke", post(handlers::clients::poke_client))
        // Berechtigungen
        .route(
            "/v1/permissions/:target",
            get(handlers::permissions::get_permissions),
        )
        .route(
            "/v1/permissions",
            post(handlers::permissions::set_permission),
        )
        .route(
            "/v1/permissions/:id",
            delete(handlers::permissions::remove_permission),
        )
        // Dateien
        .route("/v1/files/:channel_id", get(handlers::files::list_files))
        .route("/v1/files/:id", delete(handlers::files::delete_file))
        // Logs
        .route("/v1/logs", get(handlers::logs::get_logs))
}
