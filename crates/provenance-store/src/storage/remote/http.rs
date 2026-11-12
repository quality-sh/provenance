//! The HTTP/1.1 client of the remote adapter: one request on a new
//! connection, and one answer read by its declared length.
//!
//! One deadline covers the whole exchange: the connection, the request and
//! the answer. The answer ends at its `Content-Length`, or at the last chunk
//! of a `chunked` body, or with no body for `204` and `304`. Only an answer
//! with neither length nor chunks is read until the object closes the
//! connection. Interim `1xx` answers before the final answer are skipped.
//!
//! The client does not support:
//! - TLS (`https://`), proxies and redirects. A `3xx` is an answer like any
//!   other.
//! - Keep-alive: each request sends `Connection: close` and uses its own
//!   connection.
//! - `Expect: 100-continue`: the client sends the body at once.
//! - `101 Switching Protocols` and transfer codings other than `chunked`.
//!   It refuses such an answer.
//! - Folded header lines and a `Content-Length` list, which it refuses, and
//!   two `Content-Length` headers that differ, which it refuses.
//!
//! It ignores chunk extensions and trailers.

use super::{Address, Answer, Unsent};
use crate::refusal::fault;
use anyhow::Context as _;
use std::io::{Read as _, Write as _};
use std::net::{TcpStream, ToSocketAddrs as _};
use std::time::{Duration, Instant};

/// The most bytes that one line of an answer head or one chunk size line
/// can have.
const LINE_LIMIT: usize = 64 * 1024;

/// The bytes that one read or one write moves at most.
const PIECE: usize = 64 * 1024;

/// Sends one request on a new connection and reads its whole answer, all
/// within `timeout`.
pub(super) fn exchange(
    address: &Address,
    timeout: Duration,
    method: &str,
    path: &str,
    headers: &[(&str, String)],
    body: Option<&[u8]>,
) -> Result<Answer, Unsent> {
    let deadline = Instant::now() + timeout;
    let mut stream = connect(address, deadline).map_err(Unsent::Request)?;
    let mut lines = vec![
        format!("{method} {path} HTTP/1.1"),
        format!("Host: {}", address.host),
        "Connection: close".to_owned(),
        "Accept: application/json".to_owned(),
    ];
    lines.extend(
        headers
            .iter()
            .map(|(name, value)| format!("{name}: {value}")),
    );
    if let Some(body) = body {
        lines.push("Content-Type: application/json".to_owned());
        lines.push(format!("Content-Length: {}", body.len()));
    }
    let mut bytes = format!("{}\r\n\r\n", lines.join("\r\n")).into_bytes();
    bytes.extend_from_slice(body.unwrap_or_default());
    send(&mut stream, &bytes, deadline)
        .with_context(|| format!("could not send {method} {path} to {}", address.host))
        .map_err(Unsent::Request)?;
    Incoming::new(stream, deadline)
        .answer()
        .with_context(|| format!("no answer to {method} {path} from {}", address.host))
        .map_err(Unsent::Answer)
}

/// The time left before `deadline`. Refuses when none is left.
fn left(deadline: Instant) -> anyhow::Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|left| !left.is_zero())
        .ok_or_else(|| fault!("the exchange with the object passed its time limit"))
}

fn connect(address: &Address, deadline: Instant) -> anyhow::Result<TcpStream> {
    let mut last = None;
    for socket in address
        .connect
        .to_socket_addrs()
        .with_context(|| format!("could not resolve {}", address.connect))?
    {
        match TcpStream::connect_timeout(&socket, left(deadline)?) {
            Ok(stream) => return Ok(stream),
            Err(error) => last = Some(error),
        }
    }
    let error = last.map_or_else(|| fault!("{} has no address", address.connect), Into::into);
    Err(error.context(format!("could not connect to {}", address.connect)))
}

fn send(stream: &mut TcpStream, bytes: &[u8], deadline: Instant) -> anyhow::Result<()> {
    for piece in bytes.chunks(PIECE) {
        stream.set_write_timeout(Some(left(deadline)?))?;
        stream.write_all(piece)?;
    }
    stream.flush()?;
    Ok(())
}

/// How the body of an answer ends.
enum Framing {
    Length(usize),
    Chunked,
    Close,
}

/// The bytes of an answer as they arrive, read up to `deadline`.
struct Incoming {
    stream: TcpStream,
    deadline: Instant,
    buffer: Vec<u8>,
    /// The first byte of `buffer` that is not read yet.
    at: usize,
}

impl Incoming {
    const fn new(stream: TcpStream, deadline: Instant) -> Self {
        Self {
            stream,
            deadline,
            buffer: Vec::new(),
            at: 0,
        }
    }

    /// The status and body of the final answer.
    fn answer(mut self) -> anyhow::Result<Answer> {
        loop {
            let status = status(&self.line()?)?;
            let framing = self.framing()?;
            match status {
                101 => return Err(fault!("the object answered 101 Switching Protocols")),
                100..=199 => continue,
                _ => {}
            }
            let body = match (status, framing) {
                (204 | 304, _) => Vec::new(),
                (_, Framing::Length(length)) => self.take(length)?,
                (_, Framing::Chunked) => self.dechunk()?,
                (_, Framing::Close) => self.until_close()?,
            };
            return Ok(Answer { status, body });
        }
    }

    /// Reads the header lines up to the empty line, and gives how the body
    /// ends.
    fn framing(&mut self) -> anyhow::Result<Framing> {
        let mut length = None;
        let mut chunked = false;
        loop {
            let line = self.line()?;
            if line.is_empty() {
                break;
            }
            if line.starts_with([' ', '\t']) {
                return Err(fault!("the answer folds a header line"));
            }
            let (name, value) = line
                .split_once(':')
                .ok_or_else(|| fault!("an answer header has no `:`"))?;
            let value = value.trim();
            if name.eq_ignore_ascii_case("content-length") {
                let parsed = Some(value)
                    .filter(|value| !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()))
                    .and_then(|value| value.parse::<usize>().ok())
                    .ok_or_else(|| fault!("the answer has a bad Content-Length `{value}`"))?;
                if length.is_some_and(|known| known != parsed) {
                    return Err(fault!(
                        "the answer has two Content-Length headers that differ"
                    ));
                }
                length = Some(parsed);
            } else if name.eq_ignore_ascii_case("transfer-encoding") {
                if !value.eq_ignore_ascii_case("chunked") {
                    return Err(fault!(
                        "the answer has the transfer coding `{value}`; only `chunked` is read"
                    ));
                }
                chunked = true;
            }
        }
        Ok(match (chunked, length) {
            (true, _) => Framing::Chunked,
            (false, Some(length)) => Framing::Length(length),
            (false, None) => Framing::Close,
        })
    }

    fn dechunk(&mut self) -> anyhow::Result<Vec<u8>> {
        let mut body = Vec::new();
        loop {
            let line = self.line()?;
            let size = line
                .split(';')
                .next()
                .map(str::trim)
                .filter(|size| !size.is_empty() && size.bytes().all(|b| b.is_ascii_hexdigit()))
                .and_then(|size| usize::from_str_radix(size, 16).ok())
                .ok_or_else(|| fault!("a chunk has a bad size `{line}`"))?;
            if size == 0 {
                while !self.line()?.is_empty() {}
                return Ok(body);
            }
            body.extend_from_slice(&self.take(size)?);
            if self.take(2)? != b"\r\n" {
                return Err(fault!("a chunk does not end with a line break"));
            }
        }
    }

    /// The next line, with no line break.
    fn line(&mut self) -> anyhow::Result<String> {
        loop {
            if let Some(end) = find(&self.buffer[self.at..], b"\r\n") {
                let line = std::str::from_utf8(&self.buffer[self.at..self.at + end])
                    .context("an answer line is not UTF-8")?
                    .to_owned();
                self.at += end + 2;
                return Ok(line);
            }
            if self.buffer.len() - self.at > LINE_LIMIT {
                return Err(fault!("an answer line is longer than {LINE_LIMIT} bytes"));
            }
            self.more()?;
        }
    }

    /// The next `length` bytes.
    fn take(&mut self, length: usize) -> anyhow::Result<Vec<u8>> {
        while self.buffer.len() - self.at < length {
            self.more()?;
        }
        let bytes = self.buffer[self.at..self.at + length].to_vec();
        self.at += length;
        Ok(bytes)
    }

    /// Every byte up to the close of the connection.
    fn until_close(&mut self) -> anyhow::Result<Vec<u8>> {
        while self.read()? > 0 {}
        let bytes = self.buffer[self.at..].to_vec();
        self.at = self.buffer.len();
        Ok(bytes)
    }

    /// Reads more bytes. Refuses a close of the connection.
    fn more(&mut self) -> anyhow::Result<()> {
        if self.read()? == 0 {
            return Err(fault!(
                "the object closed the connection before the end of the answer"
            ));
        }
        Ok(())
    }

    /// Reads the bytes that arrive next, and gives their number: 0 when the
    /// object closed the connection.
    fn read(&mut self) -> anyhow::Result<usize> {
        if self.at > PIECE && self.at * 2 > self.buffer.len() {
            self.buffer.drain(..self.at);
            self.at = 0;
        }
        self.stream.set_read_timeout(Some(left(self.deadline)?))?;
        let start = self.buffer.len();
        self.buffer.resize(start + PIECE, 0);
        let read = self.stream.read(&mut self.buffer[start..]);
        self.buffer
            .truncate(start + read.as_ref().map_or(0, |read| *read));
        read.context("could not read the answer")
    }
}

/// The status code of a status line.
fn status(line: &str) -> anyhow::Result<u16> {
    line.strip_prefix("HTTP/1.")
        .and_then(|rest| rest.split(' ').nth(1))
        .filter(|code| code.len() == 3)
        .and_then(|code| code.parse::<u16>().ok())
        .filter(|code| *code >= 100)
        .ok_or_else(|| fault!("the answer has no status in `{line}`"))
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}
