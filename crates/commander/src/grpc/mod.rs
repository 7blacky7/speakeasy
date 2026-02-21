//! gRPC-Interface fuer den Speakeasy Commander

pub mod server;
pub mod services;

pub use server::{GrpcServer, GrpcServerKonfig};
