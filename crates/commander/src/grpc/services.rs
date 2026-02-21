//! gRPC-Service-Implementierungen fuer den Speakeasy Commander
//!
//! Alle Services nutzen denselben type-erased ExecutorFn wie der REST-Server,
//! um Send-Futures aus nicht-Send Repository-Traits zu vermeiden.

use tonic::{Request, Response, Status};
use uuid::Uuid;

use crate::commands::types::{BerechtigungsWertInput, Command};
use crate::error::CommanderError;
use crate::rest::{CommanderState, TokenValidatorFn};

// Generierter Code aus tonic-build
pub mod proto {
    tonic::include_proto!("speakeasy.v1");
}

use proto::*;

// ---------------------------------------------------------------------------
// Hilfsfunktion: Token aus gRPC-Metadaten extrahieren
// ---------------------------------------------------------------------------

fn session_aus_metadata(
    metadata: &tonic::metadata::MetadataMap,
    token_validator: &TokenValidatorFn,
) -> Result<crate::auth::CommanderSession, Status> {
    let token = metadata
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .ok_or_else(|| Status::unauthenticated("Authorization-Metadaten fehlen"))?;

    token_validator(token).map_err(|_| Status::unauthenticated("Ungueltiger oder abgelaufener Token"))
}

// ---------------------------------------------------------------------------
// ServerService
// ---------------------------------------------------------------------------

pub struct ServerServiceImpl {
    state: CommanderState,
}

impl ServerServiceImpl {
    pub fn neu(state: CommanderState) -> Self {
        Self { state }
    }
}

#[tonic::async_trait]
impl proto::server_service_server::ServerService for ServerServiceImpl {
    async fn get_server_info(
        &self,
        request: Request<Empty>,
    ) -> Result<Response<ServerInfo>, Status> {
        let session = session_aus_metadata(request.metadata(), &self.state.token_validator)?;
        match self.state.ausfuehren(Command::ServerInfo, session).await {
            Ok(crate::commands::types::Response::ServerInfo(info)) => {
                Ok(Response::new(ServerInfo {
                    server_id: Some(ServerId { value: Uuid::new_v4().to_string() }),
                    name: info.name,
                    welcome_message: info.willkommensnachricht,
                    max_clients: info.max_clients,
                    current_clients: info.aktuelle_clients,
                    version: info.version,
                    uptime_secs: info.uptime_secs,
                    host_message: String::new(),
                    default_groups: vec![],
                }))
            }
            Ok(_) => Err(Status::internal("Unerwarteter Response-Typ")),
            Err(e) => Err(commander_error_zu_status(e)),
        }
    }

    async fn update_server(
        &self,
        request: Request<UpdateServerRequest>,
    ) -> Result<Response<ServerInfo>, Status> {
        let session = session_aus_metadata(request.metadata(), &self.state.token_validator)?;
        let body = request.into_inner();
        let cmd = Command::ServerEdit {
            name: Some(body.name).filter(|s| !s.is_empty()),
            willkommensnachricht: Some(body.welcome_message).filter(|s| !s.is_empty()),
            max_clients: Some(body.max_clients).filter(|&n| n > 0),
            host_nachricht: Some(body.host_message).filter(|s| !s.is_empty()),
        };
        self.state.ausfuehren(cmd, session.clone()).await.map_err(commander_error_zu_status)?;
        match self.state.ausfuehren(Command::ServerInfo, session).await {
            Ok(crate::commands::types::Response::ServerInfo(info)) => {
                Ok(Response::new(ServerInfo {
                    server_id: Some(ServerId { value: Uuid::new_v4().to_string() }),
                    name: info.name,
                    welcome_message: info.willkommensnachricht,
                    max_clients: info.max_clients,
                    current_clients: info.aktuelle_clients,
                    version: info.version,
                    uptime_secs: info.uptime_secs,
                    host_message: String::new(),
                    default_groups: vec![],
                }))
            }
            _ => Err(Status::internal("Konnte Server-Info nicht laden")),
        }
    }

    async fn stop_server(
        &self,
        request: Request<StopServerRequest>,
    ) -> Result<Response<Empty>, Status> {
        let session = session_aus_metadata(request.metadata(), &self.state.token_validator)?;
        let body = request.into_inner();
        self.state
            .ausfuehren(
                Command::ServerStop { grund: Some(body.reason).filter(|s| !s.is_empty()) },
                session,
            )
            .await
            .map_err(commander_error_zu_status)?;
        Ok(Response::new(Empty {}))
    }

    async fn get_metrics(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<ServerMetrics>, Status> {
        Ok(Response::new(ServerMetrics {
            avg_rtt_ms: 0.0,
            packet_loss_percent: 0.0,
            avg_jitter_ms: 0.0,
            cpu_usage_percent: 0.0,
            total_bitrate_kbps: 0,
            connected_clients: 0,
            uptime_secs: 0,
        }))
    }
}

// ---------------------------------------------------------------------------
// ChannelService
// ---------------------------------------------------------------------------

pub struct ChannelServiceImpl {
    state: CommanderState,
}

impl ChannelServiceImpl {
    pub fn neu(state: CommanderState) -> Self {
        Self { state }
    }
}

#[tonic::async_trait]
impl proto::channel_service_server::ChannelService for ChannelServiceImpl {
    async fn list_channels(
        &self,
        request: Request<ListChannelsRequest>,
    ) -> Result<Response<ListChannelsResponse>, Status> {
        let session = session_aus_metadata(request.metadata(), &self.state.token_validator)?;
        match self.state.ausfuehren(Command::KanalListe, session).await {
            Ok(crate::commands::types::Response::KanalListe(kanaele)) => {
                let channels: Vec<ChannelInfo> = kanaele.into_iter().map(kanal_info_zu_proto).collect();
                let total = channels.len() as u32;
                Ok(Response::new(ListChannelsResponse {
                    channels,
                    page_info: Some(PageInfo { total_count: total, page: 1, page_size: total, has_next_page: false }),
                }))
            }
            Ok(_) => Err(Status::internal("Unerwarteter Response-Typ")),
            Err(e) => Err(commander_error_zu_status(e)),
        }
    }

    async fn get_channel(
        &self,
        _request: Request<ChannelId>,
    ) -> Result<Response<ChannelInfo>, Status> {
        Err(Status::unimplemented("GetChannel noch nicht implementiert"))
    }

    async fn create_channel(
        &self,
        request: Request<CreateChannelRequest>,
    ) -> Result<Response<CreateChannelResponse>, Status> {
        let session = session_aus_metadata(request.metadata(), &self.state.token_validator)?;
        let body = request.into_inner();
        let cmd = Command::KanalErstellen {
            name: body.name,
            parent_id: body.parent_id.and_then(|id| Uuid::parse_str(&id.value).ok()),
            thema: Some(body.description).filter(|s| !s.is_empty()),
            passwort: Some(body.password).filter(|s| !s.is_empty()),
            max_clients: body.max_clients as i64,
            sort_order: body.sort_order as i64,
            permanent: body.permanent,
        };
        match self.state.ausfuehren(cmd, session).await {
            Ok(crate::commands::types::Response::Kanal(kanal)) => {
                Ok(Response::new(CreateChannelResponse { channel: Some(kanal_info_zu_proto(kanal)) }))
            }
            Ok(_) => Err(Status::internal("Unerwarteter Response-Typ")),
            Err(e) => Err(commander_error_zu_status(e)),
        }
    }

    async fn update_channel(
        &self,
        request: Request<UpdateChannelRequest>,
    ) -> Result<Response<ChannelInfo>, Status> {
        let session = session_aus_metadata(request.metadata(), &self.state.token_validator)?;
        let body = request.into_inner();
        let id = body
            .channel_id
            .and_then(|cid| Uuid::parse_str(&cid.value).ok())
            .ok_or_else(|| Status::invalid_argument("Ungueltige channel_id"))?;
        let cmd = Command::KanalBearbeiten {
            id,
            name: Some(body.name).filter(|s| !s.is_empty()),
            thema: Some(body.description).filter(|s| !s.is_empty()).map(Some),
            max_clients: Some(body.max_clients as i64).filter(|&n| n > 0),
            sort_order: Some(body.sort_order as i64),
        };
        match self.state.ausfuehren(cmd, session).await {
            Ok(crate::commands::types::Response::Kanal(kanal)) => {
                Ok(Response::new(kanal_info_zu_proto(kanal)))
            }
            Ok(_) => Err(Status::internal("Unerwarteter Response-Typ")),
            Err(e) => Err(commander_error_zu_status(e)),
        }
    }

    async fn delete_channel(
        &self,
        request: Request<DeleteChannelRequest>,
    ) -> Result<Response<Empty>, Status> {
        let session = session_aus_metadata(request.metadata(), &self.state.token_validator)?;
        let body = request.into_inner();
        let id = body
            .channel_id
            .and_then(|cid| Uuid::parse_str(&cid.value).ok())
            .ok_or_else(|| Status::invalid_argument("Ungueltige channel_id"))?;
        self.state
            .ausfuehren(Command::KanalLoeschen { id }, session)
            .await
            .map_err(commander_error_zu_status)?;
        Ok(Response::new(Empty {}))
    }
}

// ---------------------------------------------------------------------------
// ClientService
// ---------------------------------------------------------------------------

pub struct ClientServiceImpl {
    state: CommanderState,
}

impl ClientServiceImpl {
    pub fn neu(state: CommanderState) -> Self {
        Self { state }
    }
}

#[tonic::async_trait]
impl proto::client_service_server::ClientService for ClientServiceImpl {
    async fn list_clients(
        &self,
        request: Request<ListClientsRequest>,
    ) -> Result<Response<ListClientsResponse>, Status> {
        let session = session_aus_metadata(request.metadata(), &self.state.token_validator)?;
        match self.state.ausfuehren(Command::ClientListe, session).await {
            Ok(crate::commands::types::Response::ClientListe(clients)) => {
                let proto_clients: Vec<ClientInfo> = clients.into_iter().map(client_info_zu_proto).collect();
                let total = proto_clients.len() as u32;
                Ok(Response::new(ListClientsResponse {
                    clients: proto_clients,
                    page_info: Some(PageInfo { total_count: total, page: 1, page_size: total, has_next_page: false }),
                }))
            }
            Ok(_) => Err(Status::internal("Unerwarteter Response-Typ")),
            Err(e) => Err(commander_error_zu_status(e)),
        }
    }

    async fn get_client(
        &self,
        _request: Request<UserId>,
    ) -> Result<Response<ClientInfo>, Status> {
        Err(Status::unimplemented("GetClient noch nicht implementiert"))
    }

    async fn kick_client(
        &self,
        request: Request<KickClientRequest>,
    ) -> Result<Response<Empty>, Status> {
        let session = session_aus_metadata(request.metadata(), &self.state.token_validator)?;
        let body = request.into_inner();
        let client_id = body
            .target_user_id
            .and_then(|uid| Uuid::parse_str(&uid.value).ok())
            .ok_or_else(|| Status::invalid_argument("Ungueltige user_id"))?;
        self.state
            .ausfuehren(
                Command::ClientKicken { client_id, grund: Some(body.reason).filter(|s| !s.is_empty()) },
                session,
            )
            .await
            .map_err(commander_error_zu_status)?;
        Ok(Response::new(Empty {}))
    }

    async fn ban_client(
        &self,
        request: Request<BanClientRequest>,
    ) -> Result<Response<Empty>, Status> {
        let session = session_aus_metadata(request.metadata(), &self.state.token_validator)?;
        let body = request.into_inner();
        let client_id = body
            .target_user_id
            .and_then(|uid| Uuid::parse_str(&uid.value).ok())
            .ok_or_else(|| Status::invalid_argument("Ungueltige user_id"))?;
        self.state
            .ausfuehren(
                Command::ClientBannen {
                    client_id,
                    dauer_secs: Some(body.duration_secs).filter(|&d| d > 0),
                    grund: Some(body.reason).filter(|s| !s.is_empty()),
                    ip_bannen: body.ban_ip,
                },
                session,
            )
            .await
            .map_err(commander_error_zu_status)?;
        Ok(Response::new(Empty {}))
    }

    async fn move_client(
        &self,
        request: Request<MoveClientRequest>,
    ) -> Result<Response<Empty>, Status> {
        let session = session_aus_metadata(request.metadata(), &self.state.token_validator)?;
        let body = request.into_inner();
        let client_id = body
            .target_user_id
            .and_then(|uid| Uuid::parse_str(&uid.value).ok())
            .ok_or_else(|| Status::invalid_argument("Ungueltige user_id"))?;
        let kanal_id = body
            .target_channel_id
            .and_then(|cid| Uuid::parse_str(&cid.value).ok())
            .ok_or_else(|| Status::invalid_argument("Ungueltige channel_id"))?;
        self.state
            .ausfuehren(Command::ClientVerschieben { client_id, kanal_id }, session)
            .await
            .map_err(commander_error_zu_status)?;
        Ok(Response::new(Empty {}))
    }
}

// ---------------------------------------------------------------------------
// PermissionService
// ---------------------------------------------------------------------------

pub struct PermissionServiceImpl {
    state: CommanderState,
}

impl PermissionServiceImpl {
    pub fn neu(state: CommanderState) -> Self {
        Self { state }
    }
}

#[tonic::async_trait]
impl proto::permission_service_server::PermissionService for PermissionServiceImpl {
    async fn get_permissions(
        &self,
        request: Request<GetPermissionsRequest>,
    ) -> Result<Response<GetPermissionsResponse>, Status> {
        let session = session_aus_metadata(request.metadata(), &self.state.token_validator)?;
        let body = request.into_inner();
        match self.state
            .ausfuehren(
                Command::BerechtigungListe { ziel: body.target.clone(), scope: body.scope.clone() },
                session,
            )
            .await
        {
            Ok(crate::commands::types::Response::BerechtigungListe(eintraege)) => {
                let permissions: Vec<PermissionEntry> = eintraege
                    .into_iter()
                    .map(|e| PermissionEntry {
                        permission: e.permission,
                        value: Some(berechtigung_zu_proto(e.wert)),
                    })
                    .collect();
                Ok(Response::new(GetPermissionsResponse { target: body.target, permissions }))
            }
            Ok(_) => Err(Status::internal("Unerwarteter Response-Typ")),
            Err(e) => Err(commander_error_zu_status(e)),
        }
    }

    async fn set_permission(
        &self,
        request: Request<SetPermissionRequest>,
    ) -> Result<Response<Empty>, Status> {
        let session = session_aus_metadata(request.metadata(), &self.state.token_validator)?;
        let body = request.into_inner();
        let wert = body.value.map(proto_zu_berechtigung).unwrap_or(BerechtigungsWertInput::Skip);
        self.state
            .ausfuehren(
                Command::BerechtigungSetzen {
                    ziel: body.target,
                    permission: body.permission,
                    wert,
                    scope: body.scope,
                },
                session,
            )
            .await
            .map_err(commander_error_zu_status)?;
        Ok(Response::new(Empty {}))
    }

    async fn remove_permission(
        &self,
        request: Request<RemovePermissionRequest>,
    ) -> Result<Response<Empty>, Status> {
        let session = session_aus_metadata(request.metadata(), &self.state.token_validator)?;
        let body = request.into_inner();
        self.state
            .ausfuehren(
                Command::BerechtigungEntfernen {
                    ziel: body.target,
                    permission: body.permission,
                    scope: body.scope,
                },
                session,
            )
            .await
            .map_err(commander_error_zu_status)?;
        Ok(Response::new(Empty {}))
    }
}

// ---------------------------------------------------------------------------
// FileService
// ---------------------------------------------------------------------------

pub struct FileServiceImpl {
    state: CommanderState,
}

impl FileServiceImpl {
    pub fn neu(state: CommanderState) -> Self {
        Self { state }
    }
}

#[tonic::async_trait]
impl proto::file_service_server::FileService for FileServiceImpl {
    async fn list_files(
        &self,
        request: Request<ListFilesRequest>,
    ) -> Result<Response<ListFilesResponse>, Status> {
        let session = session_aus_metadata(request.metadata(), &self.state.token_validator)?;
        let body = request.into_inner();
        let kanal_id = body
            .channel_id
            .and_then(|cid| Uuid::parse_str(&cid.value).ok())
            .ok_or_else(|| Status::invalid_argument("Ungueltige channel_id"))?;
        match self.state.ausfuehren(Command::DateiListe { kanal_id }, session).await {
            Ok(crate::commands::types::Response::DateiListe(dateien)) => {
                let files: Vec<FileEntry> = dateien
                    .into_iter()
                    .map(|d| FileEntry {
                        file_id: d.datei_id,
                        name: d.name,
                        size_bytes: d.groesse_bytes,
                        channel_id: Some(ChannelId { value: d.kanal_id.to_string() }),
                        uploaded_by: Some(UserId { value: d.hochgeladen_von.to_string() }),
                        uploaded_at_ms: d.hochgeladen_am_ms,
                        mime_type: d.mime_typ,
                        checksum_sha256: String::new(),
                    })
                    .collect();
                let total = files.len() as u32;
                Ok(Response::new(ListFilesResponse {
                    files,
                    page_info: Some(PageInfo { total_count: total, page: 1, page_size: total, has_next_page: false }),
                }))
            }
            Ok(_) => Err(Status::internal("Unerwarteter Response-Typ")),
            Err(e) => Err(commander_error_zu_status(e)),
        }
    }

    async fn initiate_upload(
        &self,
        _request: Request<InitiateUploadRequest>,
    ) -> Result<Response<InitiateUploadResponse>, Status> {
        Err(Status::unimplemented("Upload noch nicht implementiert"))
    }

    async fn delete_file(
        &self,
        request: Request<DeleteFileRequest>,
    ) -> Result<Response<Empty>, Status> {
        let session = session_aus_metadata(request.metadata(), &self.state.token_validator)?;
        let body = request.into_inner();
        self.state
            .ausfuehren(Command::DateiLoeschen { datei_id: body.file_id }, session)
            .await
            .map_err(commander_error_zu_status)?;
        Ok(Response::new(Empty {}))
    }
}

// ---------------------------------------------------------------------------
// Hilfsfunktionen
// ---------------------------------------------------------------------------

fn commander_error_zu_status(e: CommanderError) -> Status {
    match e {
        CommanderError::Authentifizierung(_) => Status::unauthenticated(e.to_string()),
        CommanderError::NichtAutorisiert(_) => Status::permission_denied(e.to_string()),
        CommanderError::NichtGefunden(_) => Status::not_found(e.to_string()),
        CommanderError::UngueltigeEingabe(_) => Status::invalid_argument(e.to_string()),
        CommanderError::RateLimitUeberschritten { .. } => Status::resource_exhausted(e.to_string()),
        _ => Status::internal(e.to_string()),
    }
}

fn kanal_info_zu_proto(k: crate::commands::types::KanalInfo) -> ChannelInfo {
    ChannelInfo {
        channel_id: Some(ChannelId { value: k.id.to_string() }),
        name: k.name,
        description: k.thema.unwrap_or_default(),
        parent_id: k.parent_id.map(|id| ChannelId { value: id.to_string() }),
        sort_order: k.sort_order as i32,
        max_clients: k.max_clients as u32,
        current_clients: k.aktuelle_clients,
        password_protected: k.passwort_geschuetzt,
        codec: "opus".to_string(),
        codec_quality: 10,
        permanent: false,
    }
}

fn client_info_zu_proto(c: crate::commands::types::ClientInfo) -> ClientInfo {
    ClientInfo {
        user_id: Some(UserId { value: c.user_id.to_string() }),
        username: c.username.clone(),
        display_name: c.username,
        channel_id: c.kanal_id.map(|id| ChannelId { value: id.to_string() }),
        server_groups: vec![],
        is_muted: c.ist_gemutet,
        is_deafened: c.ist_gehoerlos,
        is_input_muted: false,
        client_version: String::new(),
        connected_since_ms: c.verbunden_seit_ms,
        ip_address: c.ip_adresse.unwrap_or_default(),
    }
}

fn berechtigung_zu_proto(wert: BerechtigungsWertInput) -> PermissionValue {
    use proto::permission_value::Value;
    let value = match wert {
        BerechtigungsWertInput::Grant => Some(Value::Grant(true)),
        BerechtigungsWertInput::Deny => Some(Value::Deny(true)),
        BerechtigungsWertInput::Skip => Some(Value::Skip(true)),
        BerechtigungsWertInput::IntLimit(n) => Some(Value::IntLimit(n)),
    };
    PermissionValue { value }
}

fn proto_zu_berechtigung(pv: PermissionValue) -> BerechtigungsWertInput {
    use proto::permission_value::Value;
    match pv.value {
        Some(Value::Grant(_)) => BerechtigungsWertInput::Grant,
        Some(Value::Deny(_)) => BerechtigungsWertInput::Deny,
        Some(Value::Skip(_)) => BerechtigungsWertInput::Skip,
        Some(Value::IntLimit(n)) => BerechtigungsWertInput::IntLimit(n),
        None => BerechtigungsWertInput::Skip,
    }
}

// Exportiert ExecutorFn und TokenValidatorFn fuer den gRPC-Server
pub use crate::rest::{ExecutorFn as GrpcExecutorFn, TokenValidatorFn as GrpcTokenValidatorFn};
