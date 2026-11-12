//! The remote adapter: the canonical state of one session of a project, kept
//! in the project's object and reached over HTTP.
//!
//! The object serializes the publishes of a session and refuses a set whose
//! base revision is not the stored revision, so the engine's section for a
//! remote state is a lock in the caller's process, such as a
//! [`MemorySectionLock`](super::memory::MemorySectionLock). Two processes
//! that publish on the same base get one landed set and one
//! [`PublishFailure::BaseMoved`].
//!
//! The adapter speaks HTTP/1.1 with no TLS, one request for each
//! connection. The object base is an `http://` address on a private network,
//! such as the address of the objects in a container.

pub mod wire;

use super::{PublishFailure, Revision, StateStorage, StateView, StorageError, WriteSet};
use crate::refusal::fault;
use anyhow::Context as _;
use std::io::{Read as _, Write as _};
use std::net::{TcpStream, ToSocketAddrs as _};
use std::sync::{Mutex, PoisonError};
use std::time::Duration;
use wire::{PublishBody, StateBody, StateFailure};

/// How long the adapter waits to connect, to send and for the answer.
const TIMEOUT: Duration = Duration::from_secs(60);

/// The canonical state of `session` of `project` at an object base.
#[derive(Debug)]
pub struct RemoteStateStorage {
    address: Address,
    /// The path of the session, below the host.
    session: String,
    /// The last view read, sent back as `If-None-Match`.
    last: Mutex<Option<StateView>>,
}

impl RemoteStateStorage {
    /// The state of `session` of `project` at `object_base`, an `http://`
    /// address. Refuses a base of another scheme and a project or session
    /// name that [`wire::is_name`] refuses. Makes no request.
    pub fn new(object_base: &str, project: &str, session: &str) -> anyhow::Result<Self> {
        for (what, name) in [("project", project), ("session", session)] {
            if !wire::is_name(name) {
                return Err(fault!(
                    "the {what} name `{name}` is not 1 to 64 letters, digits, `_` and `-`"
                ));
            }
        }
        let address = Address::parse(object_base)?;
        Ok(Self {
            session: format!("{}{}", address.prefix, wire::session_path(project, session)),
            address,
            last: Mutex::new(None),
        })
    }

    fn last(&self) -> std::sync::MutexGuard<'_, Option<StateView>> {
        // Each change replaces the whole value, so a poisoned value is intact.
        self.last.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Sends `method` to `path` below the session path of this storage.
    pub(crate) fn send(
        &self,
        method: &str,
        path: &str,
        headers: &[(&str, String)],
        body: Option<&[u8]>,
    ) -> Result<Answer, Unsent> {
        let path = format!("{}{path}", self.session);
        exchange(&self.address, method, &path, headers, body)
    }
}

impl StateStorage for RemoteStateStorage {
    fn read_view(&self) -> Result<StateView, StorageError> {
        Err(StorageError::Read(fault!("not built")))
    }

    fn publish(&self, _writes: WriteSet) -> Result<(), PublishFailure> {
        Err(PublishFailure::Refused(fault!("not built")))
    }
}

/// The host and path prefix of an object base.
#[derive(Clone, Debug)]
struct Address {
    /// `host:port`, to connect to.
    connect: String,
    /// The authority as the base gives it, for the `Host` header.
    host: String,
    /// The path of the base with no final `/`, or `""`.
    prefix: String,
}

impl Address {
    fn parse(base: &str) -> anyhow::Result<Self> {
        let rest = base
            .strip_prefix("http://")
            .ok_or_else(|| fault!("the object base `{base}` does not start with `http://`"))?;
        let (host, prefix) = rest.split_once('/').unwrap_or((rest, ""));
        if host.is_empty() || host.contains('@') {
            return Err(fault!(
                "the object base `{base}` has no host, or has user information"
            ));
        }
        let has_port = host
            .rsplit_once(':')
            .is_some_and(|(_, port)| !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()));
        let prefix = prefix.trim_end_matches('/');
        Ok(Self {
            connect: if has_port {
                host.to_owned()
            } else {
                format!("{host}:80")
            },
            host: host.to_owned(),
            prefix: if prefix.is_empty() {
                String::new()
            } else {
                format!("/{prefix}")
            },
        })
    }
}

/// An HTTP answer: its status and its whole body.
#[derive(Debug)]
pub(crate) struct Answer {
    pub(crate) status: u16,
    pub(crate) body: Vec<u8>,
}

impl Answer {
    fn failure(&self) -> Option<StateFailure> {
        serde_json::from_slice(&self.body).ok()
    }

    /// The failure of the answer, with its status and body.
    pub(crate) fn failure_error(&self) -> anyhow::Error {
        fault!(
            "the object answered {}: {}",
            self.status,
            String::from_utf8_lossy(&self.body)
        )
    }
}

/// Why an exchange got no answer.
#[derive(Debug)]
pub(crate) enum Unsent {
    /// The request did not go: the object cannot have done it.
    Request(anyhow::Error),
    /// The request went and no whole answer came back.
    Answer(anyhow::Error),
}

impl Unsent {
    pub(crate) fn into_error(self) -> anyhow::Error {
        match self {
            Self::Request(error) | Self::Answer(error) => error,
        }
    }

    fn into_publish_failure(self) -> PublishFailure {
        match self {
            Self::Request(error) => PublishFailure::Refused(error),
            Self::Answer(error) => PublishFailure::Started(error),
        }
    }
}

/// Sends one request on a new connection and reads the whole answer.
fn exchange(
    address: &Address,
    method: &str,
    path: &str,
    headers: &[(&str, String)],
    body: Option<&[u8]>,
) -> Result<Answer, Unsent> {
    let mut stream = connect(address).map_err(Unsent::Request)?;
    let mut request = format!(
        "{method} {path} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nAccept: application/json\r\n",
        address.host
    );
    for (name, value) in headers {
        request.push_str(&format!("{name}: {value}\r\n"));
    }
    if let Some(body) = body {
        request.push_str("Content-Type: application/json\r\n");
        request.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    request.push_str("\r\n");
    let mut bytes = request.into_bytes();
    bytes.extend_from_slice(body.unwrap_or_default());
    stream
        .write_all(&bytes)
        .and_then(|()| stream.flush())
        .with_context(|| format!("could not send {method} {path} to {}", address.host))
        .map_err(Unsent::Request)?;
    let mut raw = Vec::new();
    stream
        .read_to_end(&mut raw)
        .context("could not read the answer")
        .and_then(|_| parse_answer(&raw))
        .with_context(|| format!("no answer to {method} {path} from {}", address.host))
        .map_err(Unsent::Answer)
}

fn connect(address: &Address) -> anyhow::Result<TcpStream> {
    let mut last = None;
    for socket in address
        .connect
        .to_socket_addrs()
        .with_context(|| format!("could not resolve {}", address.connect))?
    {
        match TcpStream::connect_timeout(&socket, TIMEOUT) {
            Ok(stream) => {
                stream.set_read_timeout(Some(TIMEOUT))?;
                stream.set_write_timeout(Some(TIMEOUT))?;
                return Ok(stream);
            }
            Err(error) => last = Some(error),
        }
    }
    let error = last.map_or_else(|| fault!("{} has no address", address.connect), Into::into);
    Err(error.context(format!("could not connect to {}", address.connect)))
}

/// The status and body of a whole HTTP/1.1 answer.
fn parse_answer(raw: &[u8]) -> anyhow::Result<Answer> {
    let end = find(raw, b"\r\n\r\n").ok_or_else(|| fault!("the answer has no end of its head"))?;
    let head = std::str::from_utf8(&raw[..end]).context("the answer head is not UTF-8")?;
    let mut lines = head.split("\r\n");
    let status = lines
        .next()
        .and_then(|line| line.split(' ').nth(1))
        .and_then(|code| code.parse::<u16>().ok())
        .ok_or_else(|| fault!("the answer has no status"))?;
    let mut length = None;
    let mut chunked = false;
    for line in lines {
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| fault!("an answer header has no `:`"))?;
        let value = value.trim();
        if name.eq_ignore_ascii_case("content-length") {
            length = Some(value.parse::<usize>().context("a bad Content-Length")?);
        } else if name.eq_ignore_ascii_case("transfer-encoding") {
            chunked = value.eq_ignore_ascii_case("chunked");
        }
    }
    let rest = &raw[end + 4..];
    let body = match (chunked, length) {
        (true, _) => dechunk(rest)?,
        (false, Some(length)) => rest
            .get(..length)
            .ok_or_else(|| fault!("the answer body ended early"))?
            .to_vec(),
        (false, None) => rest.to_vec(),
    };
    Ok(Answer { status, body })
}

/// The body of a chunked answer.
fn dechunk(mut rest: &[u8]) -> anyhow::Result<Vec<u8>> {
    let mut body = Vec::new();
    loop {
        let end = find(rest, b"\r\n").ok_or_else(|| fault!("a chunk has no size line"))?;
        let size = std::str::from_utf8(&rest[..end])
            .ok()
            .and_then(|line| line.split(';').next())
            .and_then(|size| usize::from_str_radix(size.trim(), 16).ok())
            .ok_or_else(|| fault!("a chunk has a bad size"))?;
        rest = &rest[end + 2..];
        if size == 0 {
            return Ok(body);
        }
        if rest.len() < size + 2 {
            return Err(fault!("the answer body ended early"));
        }
        body.extend_from_slice(&rest[..size]);
        rest = &rest[size + 2..];
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_base_gives_its_host_port_and_path_prefix() {
        let address = Address::parse("http://objects.internal/kernel/").unwrap();
        assert_eq!(address.connect, "objects.internal:80");
        assert_eq!(address.host, "objects.internal");
        assert_eq!(address.prefix, "/kernel");
        let address = Address::parse("http://127.0.0.1:8080").unwrap();
        assert_eq!(address.connect, "127.0.0.1:8080");
        assert_eq!(address.prefix, "");
        for base in ["https://a", "http://", "http://u@a", "a:1"] {
            assert!(Address::parse(base).is_err(), "{base}");
        }
    }

    #[test]
    fn the_storage_refuses_a_bad_project_or_session_name() {
        assert!(RemoteStateStorage::new("http://a", "a/b", "main").is_err());
        assert!(RemoteStateStorage::new("http://a", "a", "").is_err());
        let storage = RemoteStateStorage::new("http://a/base", "p", "main").unwrap();
        assert_eq!(storage.session, "/base/projects/p/sessions/main");
    }

    #[test]
    fn an_answer_gives_its_status_and_body() {
        let answer = parse_answer(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}").unwrap();
        assert_eq!((answer.status, answer.body.as_slice()), (200, &b"{}"[..]));
        let chunked =
            b"HTTP/1.1 404 Not Found\r\ntransfer-encoding: chunked\r\n\r\n2\r\n{}\r\n0\r\n\r\n";
        let answer = parse_answer(chunked).unwrap();
        assert_eq!((answer.status, answer.body.as_slice()), (404, &b"{}"[..]));
        assert!(parse_answer(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\n{}").is_err());
        assert!(parse_answer(b"HTTP/1.1 200 OK\r\n").is_err());
    }
}
