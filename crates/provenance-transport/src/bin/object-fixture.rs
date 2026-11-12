//! A stand-in for the projects' objects: the state wire of
//! `provenance_store::storage::remote::wire` over projects held in memory.
//!
//! The fixture prints `{"url": <base>}` on one line, then serves until its
//! standard input closes. `PUT <base>/projects/<p>/sessions/main/state` makes
//! a project while the fixture runs. Session `main` is the only session:
//! another session is an unknown target.
//!
//! Only the fixture has `PUT <session>/failing-key {"key": <key>}`. It makes
//! each later landing of that key fail, as the storage contract needs.

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::put;
use axum::Router;
use provenance_store::storage::remote::wire::{
    self, PublishBody, SeedBody, StateBody, StateFailure, MAIN_SESSION,
};
use provenance_store::storage::{Atomicity, Revision, StateKey, WriteSet};
use serde::de::DeserializeOwned;
use serde_json::json;
use std::collections::{BTreeMap, HashMap};
use std::io::Write as _;
use std::sync::{Arc, Mutex, PoisonError};
use tokio::io::AsyncReadExt as _;

/// The state of one project, in session `main`.
#[derive(Debug, Default)]
struct Project {
    entries: BTreeMap<StateKey, Vec<u8>>,
    failing: Option<StateKey>,
}

/// Every project, by name. One lock serializes every request.
type Projects = Arc<Mutex<HashMap<String, Project>>>;

/// The fixture's answer to a request.
type Answer = Result<Response, StateFailure>;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let url = format!("http://{}", listener.local_addr()?);
    println!("{}", json!({ "url": url }));
    std::io::stdout().flush()?;
    axum::serve(listener, router())
        .with_graceful_shutdown(async {
            let mut buffer = [0; 256];
            while matches!(tokio::io::stdin().read(&mut buffer).await, Ok(n) if n > 0) {}
        })
        .await?;
    Ok(())
}

fn router() -> Router {
    Router::new()
        .route(
            "/projects/{project}/sessions/{session}/state",
            put(seed).get(read).post(publish),
        )
        .route(
            "/projects/{project}/sessions/{session}/failing-key",
            put(fail_on),
        )
        .with_state(Projects::default())
}

/// `GET`: the state of the project, or `304` when `If-None-Match` names it.
async fn read(
    State(projects): State<Projects>,
    Path((project, session)): Path<(String, String)>,
    headers: HeaderMap,
) -> Response {
    answer(with_project(&projects, &project, &session, |stored| {
        let revision = Revision::of(&stored.entries);
        let tag = wire::entity_tag(revision.as_str());
        let etag = [(header::ETAG, tag.clone())];
        if headers
            .get(header::IF_NONE_MATCH)
            .is_some_and(|value| value.as_bytes() == tag.as_bytes())
        {
            return Ok((StatusCode::NOT_MODIFIED, etag).into_response());
        }
        let body = StateBody {
            revision: revision.as_str().to_owned(),
            entries: wire::encode_entries(&stored.entries),
        };
        Ok((etag, axum::Json(body)).into_response())
    }))
}

/// `PUT`: replaces the whole state, and makes the project when it does not
/// exist.
async fn seed(
    State(projects): State<Projects>,
    Path((project, session)): Path<(String, String)>,
    body: Bytes,
) -> Response {
    let entries = ensure_target(&project, &session)
        .and_then(|()| parse::<SeedBody>(&body))
        .and_then(|seed| wire::decode_entries(seed.entries).map_err(invalid));
    answer(entries.map(|entries| {
        lock(&projects).entry(project).or_default().entries = entries;
        StatusCode::NO_CONTENT.into_response()
    }))
}

/// `POST`: lands a write set, as the memory adapter does.
async fn publish(
    State(projects): State<Projects>,
    Path((project, session)): Path<(String, String)>,
    body: Bytes,
) -> Response {
    let decoded = parse::<PublishBody>(&body).and_then(|body| body.decode().map_err(invalid));
    answer(decoded.and_then(|(base, writes)| {
        with_project(&projects, &project, &session, |stored| {
            land(stored, base.as_deref(), &writes)?;
            Ok(StatusCode::NO_CONTENT.into_response())
        })
    }))
}

/// `PUT <session>/failing-key`: makes each later landing of a key fail.
async fn fail_on(
    State(projects): State<Projects>,
    Path((project, session)): Path<(String, String)>,
    body: Bytes,
) -> Response {
    #[derive(serde::Deserialize)]
    struct Failing {
        key: String,
    }
    let key = parse::<Failing>(&body)
        .and_then(|failing| StateKey::new(failing.key).map_err(|error| invalid(error.to_string())));
    answer(key.and_then(|key| {
        with_project(&projects, &project, &session, |stored| {
            stored.failing = Some(key);
            Ok(StatusCode::NO_CONTENT.into_response())
        })
    }))
}

/// Lands `writes` on `stored`: refuses a set that puts and deletes one key
/// and a set on another base, then lands the keys in key order. At the
/// failing key, a whole set lands no key and a per-key set keeps the keys
/// before it.
fn land(stored: &mut Project, base: Option<&str>, writes: &WriteSet) -> Result<(), StateFailure> {
    writes
        .ensure_disjoint()
        .map_err(|error| StateFailure::Refused {
            message: error.to_string(),
        })?;
    let current = Revision::of(&stored.entries);
    if let Some(base) = base.filter(|base| *base != current.as_str()) {
        return Err(StateFailure::BaseMoved {
            base: base.to_owned(),
            current: current.as_str().to_owned(),
        });
    }
    let mut changes: BTreeMap<&StateKey, Option<&Vec<u8>>> = BTreeMap::new();
    changes.extend(writes.deletes().iter().map(|key| (key, None)));
    changes.extend(writes.puts().iter().map(|(key, bytes)| (key, Some(bytes))));
    let mut landed = stored.entries.clone();
    for (key, bytes) in changes {
        if stored.failing.as_ref() == Some(key) {
            let message = format!("the key `{}` failed to land", key.as_str());
            return Err(match writes.atomicity() {
                Atomicity::WholeSet => StateFailure::Refused { message },
                Atomicity::PerKey => {
                    stored.entries = landed;
                    StateFailure::Started { message }
                }
            });
        }
        match bytes {
            Some(bytes) => landed.insert(key.clone(), bytes.clone()),
            None => landed.remove(key),
        };
    }
    stored.entries = landed;
    Ok(())
}

/// Runs `work` on the stored project, or refuses an unknown target.
fn with_project(
    projects: &Projects,
    project: &str,
    session: &str,
    work: impl FnOnce(&mut Project) -> Answer,
) -> Answer {
    ensure_target(project, session)?;
    lock(projects)
        .get_mut(project)
        .map_or(Err(StateFailure::UnknownTarget {}), work)
}

/// Refuses a session other than `main` and a name that cannot be a project.
fn ensure_target(project: &str, session: &str) -> Result<(), StateFailure> {
    if wire::is_name(project) && session == MAIN_SESSION {
        Ok(())
    } else {
        Err(StateFailure::UnknownTarget {})
    }
}

fn parse<T: DeserializeOwned>(body: &[u8]) -> Result<T, StateFailure> {
    serde_json::from_slice(body).map_err(|error| invalid(error.to_string()))
}

const fn invalid(message: String) -> StateFailure {
    StateFailure::InvalidInput { message }
}

fn lock(projects: &Projects) -> std::sync::MutexGuard<'_, HashMap<String, Project>> {
    // Each change replaces whole values, so a poisoned map is intact.
    projects.lock().unwrap_or_else(PoisonError::into_inner)
}

fn answer(answer: Answer) -> Response {
    answer.unwrap_or_else(|failure| {
        let status = StatusCode::from_u16(failure.status()).expect("a valid failure status");
        (status, axum::Json(failure)).into_response()
    })
}
