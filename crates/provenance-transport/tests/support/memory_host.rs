//! A repository whose state is in memory, made from the state of a fixture
//! repository in a folder, its binding with a read operations port, and the
//! form in which the answers of two hosts compare.
//!
//! Two answers compare in their status, their headers and their body bytes.
//! The parts that may differ, and why:
//! - a generated value (a record id, a digest of a record that holds one, a
//!   time, a page cursor) is random in each run, or signed with the key of
//!   one read database, so it is named by its first place in the run of its
//!   host;
//! - the `instance_id` of the projection stamp and of a `stale` refusal
//!   names one read database, and each repository has its own;
//! - the `digest` of the projection covers the records, and a record that a
//!   call made has a random id, so the digest is named as a generated value:
//!   two answers of one run that have the same digest must have the same
//!   digest in the other run.
//!
//! Each other part of the projection stamp and of a refusal compares as it
//! is: the policy, the serial, the live and attested parts, the derivation
//! and the moved units.

#[path = "../../../provenance-store/tests/support/sqlite_database.rs"]
mod sqlite_database;

use super::names::Names;
use super::router_answer::Answer;
use camino::Utf8Path;
use provenance_core::ScopeId;
use provenance_store::operations::read_operations::{
    InProcessReadOperations, ReadAnswer, ReadOperations,
};
use provenance_store::operations::LocalRepository;
use provenance_store::read_database::contract::Other;
use provenance_store::repository::{Repository as Opened, RepositoryIdentity};
use provenance_store::settings::Settings;
use provenance_store::storage::memory::{MemorySectionLock, MemoryStateStorage};
use provenance_store::storage::StateKey;
use provenance_transport::fixture::records::Repository;
use serde_json::{json, Value};
use sqlite_database::MemorySqlite;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

/// A port that keeps the name of each operation it runs, then gives the
/// call to another port.
#[derive(Debug)]
pub struct Counted {
    reads: Arc<dyn ReadOperations>,
    runs: Mutex<Vec<&'static str>>,
}

impl Counted {
    pub fn new(reads: Arc<dyn ReadOperations>) -> Arc<Self> {
        Arc::new(Self {
            reads,
            runs: Mutex::default(),
        })
    }

    /// The operations that the port ran, in order.
    pub fn runs(&self) -> Vec<&'static str> {
        self.runs.lock().unwrap().clone()
    }
}

impl ReadOperations for Counted {
    fn run(&self, operation: &'static str, request: Value) -> ReadAnswer {
        self.runs.lock().unwrap().push(operation);
        self.reads.run(operation, request)
    }
}

/// A repository over memory storage that holds the canonical state of
/// `folder`, with its settings, at the working tree `root`, which need not
/// exist. Its read database is an `SQLite` database in memory, behind the
/// contract's wrapper. The storage is given too, so a test can change the
/// state as a hand edit of the folder does.
pub async fn memory_repository(
    folder: &Repository,
    root: &Utf8Path,
) -> (Opened, Arc<MemoryStateStorage>) {
    let settings = Settings::load(&folder.layout).unwrap();
    let identity =
        RepositoryIdentity::new(root.to_owned(), ScopeId::new("default").unwrap(), settings);
    let storage = Arc::new(MemoryStateStorage::new(state(folder)));
    let repository = Opened::with_storage(
        identity,
        storage.clone(),
        Arc::new(MemorySectionLock::default()),
        Some(Arc::new(Other::new(MemorySqlite::new().await))),
    )
    .unwrap();
    (repository, storage)
}

/// The state files of `repo`, keyed as the storage keys them.
pub fn state(repo: &Repository) -> BTreeMap<StateKey, Vec<u8>> {
    let state = repo.layout.state_dir().into_std_path_buf();
    let mut entries = BTreeMap::new();
    let mut pending = vec![state.clone()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            let key = path.strip_prefix(&state).unwrap().to_str().unwrap();
            let key = StateKey::new(key.replace('\\', "/")).unwrap();
            entries.insert(key, std::fs::read(&path).unwrap());
        }
    }
    entries
}

/// The binding of `repository` under the target name `selected`, with
/// `port` as the port of its read operations.
pub fn bound(repository: Opened, port: Arc<dyn ReadOperations>) -> LocalRepository {
    LocalRepository::bind(repository, "selected")
        .unwrap()
        .with_read_operations(port)
}

/// The in-process port of `repository` under the target name `selected`,
/// counted.
pub fn in_process(repository: &Opened) -> Arc<Counted> {
    let binding = LocalRepository::bind(repository.clone(), "selected").unwrap();
    Counted::new(Arc::new(InProcessReadOperations::new(&binding)))
}

/// One answer in the form in which two hosts compare it. `names` names the
/// generated values of one host.
pub fn compared(names: &mut Names, answer: &Answer) -> Value {
    let text = std::str::from_utf8(&answer.body).unwrap();
    let body: Value = serde_json::from_str(text).unwrap();
    let headers: Vec<String> = answer
        .headers
        .iter()
        .map(|(name, value)| {
            let tag = value.trim_matches('"');
            format!("{name}: {}", value.replace(tag, &names.name(tag)))
        })
        .collect();
    let literal = ["/meta/stamp", "/error"].map(|pointer| {
        let mut part = body.pointer(pointer).cloned().unwrap_or(Value::Null);
        if let Some(part) = part.as_object_mut() {
            part.remove("instance_id");
            if let Some(Value::String(digest)) = part.get_mut("digest") {
                *digest = names.name(digest);
            }
        }
        part
    });
    json!({
        "status": answer.status,
        "headers": headers,
        "literal": literal,
        "body": named_text(names, &body, text),
    })
}

/// The body text with each generated string named and each `created_at`
/// number as `<time>`. Every other byte stays.
fn named_text(names: &mut Names, body: &Value, text: &str) -> String {
    let mut strings = Vec::new();
    collect(body, &mut strings);
    let mut text = text.to_owned();
    for string in strings {
        let name = names.name(&string);
        if name != string {
            text = text.replace(&format!("\"{string}\""), &format!("\"{name}\""));
        }
    }
    let mut out = String::new();
    let mut rest = text.as_str();
    let key = "\"created_at\":";
    while let Some(at) = rest.find(key) {
        let (head, tail) = rest.split_at(at + key.len());
        out.push_str(head);
        let digits = tail.bytes().take_while(u8::is_ascii_digit).count();
        if digits > 0 {
            out.push_str("\"<time>\"");
        }
        rest = &tail[digits..];
    }
    out.push_str(rest);
    out
}

/// Each string of `value`, in the order of a walk of its keys.
fn collect(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::String(text) => out.push(text.clone()),
        Value::Array(items) => items.iter().for_each(|item| collect(item, out)),
        Value::Object(fields) => fields.values().for_each(|field| collect(field, out)),
        _ => {}
    }
}
