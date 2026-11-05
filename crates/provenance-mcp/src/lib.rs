//! This crate is an implementation part of Provenance. Provenance is pre-release, and this API can
//! change without compatibility shims. Use `provenance-sdk` for Rust.
//!
//! The MCP server for one statement host. This is the only crate that depends on `rmcp`.
//!
//! This library does not open a listener. The caller owns the stream that `serve` reads.

mod arguments;
#[cfg(feature = "client")]
mod client;
mod io;
mod json_schema;
mod listed;
mod server;
mod surface;
mod tools;

#[cfg(feature = "client")]
pub use client::Client;
pub use rmcp::service::ServerInitializeError as ServeError;
pub use server::Server;

use provenance_transport::StatementHost;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::task::JoinError;

/// One running MCP session over one caller-owned stream.
pub struct Session(rmcp::service::RunningService<rmcp::RoleServer, Server>);

impl Session {
    /// Wait until the client closes the session.
    pub async fn waiting(self) -> Result<(), JoinError> {
        self.0.waiting().await.map(drop)
    }

    /// Stop the session and wait for its task to end.
    pub async fn cancel(self) -> Result<(), JoinError> {
        self.0.cancel().await.map(drop)
    }
}

/// Serve a bounded MCP session for one host on a caller-owned stream.
///
/// The library's initialization error is a large enum, so the failure channel boxes it to
/// keep this result small.
pub async fn serve<T>(host: StatementHost, io: T) -> Result<Session, Box<ServeError>>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    // The session setup future is large, so it lives on the heap.
    Box::pin(rmcp::ServiceExt::serve(
        Server::new(host),
        io::BoundedIo::new(io),
    ))
    .await
    .map(Session)
    .map_err(Box::new)
}

/// Serve a bounded MCP session for one host on standard input and output.
///
/// The session ends when standard input closes.
pub async fn serve_stdio(host: StatementHost) -> Result<Session, Box<ServeError>> {
    serve(
        host,
        tokio::io::join(tokio::io::stdin(), tokio::io::stdout()),
    )
    .await
}
