//! gRPC-Server fuer den Speakeasy Commander

use std::net::SocketAddr;

use anyhow::Result;
use tonic::transport::Server;

use crate::grpc::services::{
    proto::{
        channel_service_server::ChannelServiceServer,
        client_service_server::ClientServiceServer,
        file_service_server::FileServiceServer,
        permission_service_server::PermissionServiceServer,
        server_service_server::ServerServiceServer,
    },
    ChannelServiceImpl, ClientServiceImpl, FileServiceImpl, PermissionServiceImpl,
    ServerServiceImpl,
};
use crate::rest::CommanderState;

/// gRPC-Server-Konfiguration
#[derive(Debug, Clone)]
pub struct GrpcServerKonfig {
    pub bind_addr: SocketAddr,
}

impl Default for GrpcServerKonfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:9302".parse().unwrap(),
        }
    }
}

/// gRPC-Commander-Server
pub struct GrpcServer {
    konfig: GrpcServerKonfig,
}

impl GrpcServer {
    pub fn neu(konfig: GrpcServerKonfig) -> Self {
        Self { konfig }
    }

    /// Startet den gRPC-Server mit dem gegebenen CommanderState
    pub async fn starten(self, state: CommanderState) -> Result<()> {
        tracing::info!(addr = %self.konfig.bind_addr, "gRPC-Commander-Server gestartet");

        Server::builder()
            .add_service(ServerServiceServer::new(ServerServiceImpl::neu(state.clone())))
            .add_service(ChannelServiceServer::new(ChannelServiceImpl::neu(state.clone())))
            .add_service(ClientServiceServer::new(ClientServiceImpl::neu(state.clone())))
            .add_service(PermissionServiceServer::new(PermissionServiceImpl::neu(state.clone())))
            .add_service(FileServiceServer::new(FileServiceImpl::neu(state)))
            .serve(self.konfig.bind_addr)
            .await?;

        Ok(())
    }
}
