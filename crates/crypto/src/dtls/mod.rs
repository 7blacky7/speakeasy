//! DTLS Transport-Verschluesselung (Client <-> Server)
//!
//! Der Server kann Audio-Inhalte lesen (kein E2E).
//! Implementiert als TLS-Wrapper ueber TCP Control-Channel.
//!
//! ## TODO
//! - Echtes DTLS ueber UDP (Phase 6, benoetigt FFI zu OpenSSL oder mbedtls)

pub mod client;
pub mod config;
pub mod server;

pub use client::DtlsClient;
pub use config::{
    compute_certificate_fingerprint, generate_self_signed_cert, DtlsClientConfig,
    DtlsServerConfig,
};
pub use server::DtlsServer;
