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
//! connection; [`http`] gives what its client does not support. The object
//! base is an `http://` address on a private network, such as the address of
//! the objects in a container.

mod http;
pub mod wire;

use super::{PublishFailure, Revision, StateStorage, StateView, StorageError, WriteSet};
use crate::refusal::fault;
use anyhow::Context as _;
use std::sync::{Mutex, PoisonError};
use std::time::Duration;
use wire::{PublishBody, StateBody, StateFailure};

/// The time limit of one exchange: to connect, to send the request and to
/// read the whole answer.
const TIMEOUT: Duration = Duration::from_secs(60);

/// The canonical state of `session` of `project` at an object base.
#[derive(Debug)]
pub struct RemoteStateStorage {
    address: Address,
    /// The path of the session, below the host.
    session: String,
    /// The time limit of one exchange.
    timeout: Duration,
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
            timeout: TIMEOUT,
            last: Mutex::new(None),
        })
    }

    /// The same storage with another time limit for each exchange.
    #[cfg(test)]
    const fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
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
        http::exchange(&self.address, self.timeout, method, &path, headers, body)
    }
}

impl StateStorage for RemoteStateStorage {
    fn read_view(&self) -> Result<StateView, StorageError> {
        let last = self.last().clone();
        let headers: Vec<_> = last
            .iter()
            .map(|view| ("If-None-Match", wire::entity_tag(view.revision().as_str())))
            .collect();
        let answer = self
            .send("GET", "/state", &headers, None)
            .map_err(|failure| StorageError::Read(failure.into_error()))?;
        let view = match answer.status {
            304 => last.ok_or_else(|| fault!("the object answered 304 to a first read")),
            200 => serde_json::from_slice::<StateBody>(&answer.body)
                .context("the object answered a state body that is not valid")
                .and_then(|body| body.decode().map_err(|error| fault!("{error}")))
                .map(StateView::new),
            _ => Err(answer.failure_error()),
        }
        .map_err(|error| StorageError::Read(error.context("could not read the remote state")))?;
        *self.last() = Some(view.clone());
        Ok(view)
    }

    /// Refuses a set that puts and deletes one key before it sends the set.
    /// A failure to connect or to send refuses the set. A failure after the
    /// set was sent is [`PublishFailure::Started`]: the object can have
    /// landed it.
    fn publish(&self, writes: WriteSet) -> Result<(), PublishFailure> {
        writes.ensure_disjoint()?;
        let body = serde_json::to_vec(&PublishBody::of(&writes))
            .map_err(|error| PublishFailure::Refused(error.into()))?;
        let answer = self
            .send("POST", "/state", &[], Some(&body))
            .map_err(Unsent::into_publish_failure)?;
        if answer.status == 204 {
            return Ok(());
        }
        Err(match answer.failure() {
            Some(StateFailure::BaseMoved { base, current }) => PublishFailure::BaseMoved {
                base: writes
                    .base()
                    .cloned()
                    .unwrap_or_else(|| Revision::named(base)),
                current: Revision::named(current),
            },
            Some(
                StateFailure::UnknownTarget {}
                | StateFailure::InvalidInput { .. }
                | StateFailure::Refused { .. },
            ) => PublishFailure::Refused(answer.failure_error()),
            None if (400..500).contains(&answer.status) => {
                PublishFailure::Refused(answer.failure_error())
            }
            Some(StateFailure::Started { .. }) | None => {
                PublishFailure::Started(answer.failure_error())
            }
        })
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
    /// Refuses a base that does not start with `http://`, and a base with
    /// a space, a control character, `?` or `#`, which would break the
    /// request line or the `Host` header.
    fn parse(base: &str) -> anyhow::Result<Self> {
        let rest = base
            .strip_prefix("http://")
            .ok_or_else(|| fault!("the object base `{base}` does not start with `http://`"))?;
        if rest
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || c == '?' || c == '#')
        {
            return Err(fault!(
                "the object base {base:?} has a space, a control character, `?` or `#`"
            ));
        }
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

#[cfg(test)]
mod tests;
