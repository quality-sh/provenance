//! Connections of the review listener.
//!
//! The host can refuse a request before it reads the body, so that it never decodes a body
//! from an unauthenticated client. When the host then closes a socket that still has unread
//! request bytes, the operating system answers with a reset. The reset removes the part of
//! the response that the client did not read yet. On macOS the client sees
//! `Invalid argument (os error 22)`. For this reason, a closed connection ends its write side.
//! Then it reads and drops what the client still sends, up to the request body limit and for
//! 2 seconds, before it releases the socket.

use provenance_macros::rule;
use std::{
    io,
    net::SocketAddr,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf},
    net::{TcpListener, TcpStream},
};

/// The bytes a closed connection drops: the request body limit, so that a refused body within
/// the limit does not cause a reset.
const DRAIN_BYTES: usize = provenance_transport::MAX_BODY_BYTES;
/// The time a closed connection waits for the client to end its side.
const DRAIN_TIME: Duration = Duration::from_secs(2);

/// The review listener, whose connections drain at close.
pub(super) struct Listener(pub(super) TcpListener);

impl axum::serve::Listener for Listener {
    type Io = Connection;
    type Addr = SocketAddr;

    async fn accept(&mut self) -> (Connection, SocketAddr) {
        let (stream, address) = axum::serve::Listener::accept(&mut self.0).await;
        (Connection(Some(stream)), address)
    }

    fn local_addr(&self) -> io::Result<SocketAddr> {
        self.0.local_addr()
    }
}

pub(super) struct Connection(Option<TcpStream>);

impl Connection {
    const fn stream(&mut self) -> Pin<&mut TcpStream> {
        Pin::new(self.0.as_mut().expect("open connection"))
    }
}

impl AsyncRead for Connection {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        self.stream().poll_read(cx, buf)
    }
}

impl AsyncWrite for Connection {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.stream().poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.stream().poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.stream().poll_shutdown(cx)
    }

    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        self.stream().poll_write_vectored(cx, bufs)
    }

    fn is_write_vectored(&self) -> bool {
        self.0.as_ref().is_some_and(TcpStream::is_write_vectored)
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        if let (Some(stream), Ok(runtime)) = (self.0.take(), tokio::runtime::Handle::try_current())
        {
            runtime.spawn(drain(stream));
        }
    }
}

/// Reads and drops the bytes a closed connection still gets, and does not decode them.
#[rule("rule_review_refusal_reaches_client")]
async fn drain(mut stream: TcpStream) {
    let _ = stream.shutdown().await;
    let mut buffer = [0; 8192];
    while let Ok(1..) = stream.read(&mut buffer).await {}
    let _ = (DRAIN_TIME, DRAIN_BYTES);
}
