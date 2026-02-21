#![allow(clippy::too_many_arguments, clippy::result_large_err)]
//! speakeasy-commander – Commander-Interface fuer den Speakeasy Server
//!
//! Implementiert drei Zugangsarten zum Server:
//! - **REST** (/v1/...): Axum HTTP-Server mit JSON-API
//! - **TCP/TLS**: Line-based ServerQuery-Stil ueber TLS
//! - **gRPC**: Tonic-basierte High-Performance-API
//!
//! Alle drei Interfaces nutzen denselben [`commands::CommandExecutor`].

pub mod auth;
pub mod commands;
pub mod error;
pub mod grpc;
pub mod rate_limit;
pub mod rest;
pub mod tcp;

pub use error::{CommanderError, CommanderResult};
pub use commands::executor::CommandExecutor;
pub use rate_limit::{RateLimiter, RateLimitKonfig};
