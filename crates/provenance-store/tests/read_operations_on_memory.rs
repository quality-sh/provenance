//! The read operations over memory storage, with a read database that only
//! the embedder catches up.
//!
//! Implementation aid: no Rule in the graph covers a read without `host`.
//! These tests show what an embedder gets. The tests run with and without
//! `host`. Without it a read takes no publication guard and never writes
//! the read database: it answers from the database as the last
//! `catch_up_in_publish` left it, and its stamp says `annotate_only`. With
//! `host` a read catches the database up itself.
//!
//! The read database is an `SQLite` database in memory behind the
//! contract's wrapper, so the reads reach it through the port only. The
//! section lock is the memory lock that refuses a held section, so a test
//! that passes took no lock that was held.

#[path = "read_operations_on_memory/answers.rs"]
mod answers;
#[path = "read_operations_on_memory/catch_up.rs"]
mod catch_up;
#[path = "support/memory.rs"]
mod memory;
#[path = "support/sqlite_database.rs"]
mod sqlite_database;

use memory::{initial_state, scope, Recorded};
use provenance_core::{RequirementStatus, StableId};
use provenance_store::operations::read_operations::{InProcessReadOperations, ReadOperations};
use provenance_store::operations::LocalRepository;
use provenance_store::read_database::contract::Other;
use provenance_store::read_database::{ReadDatabase, ReadWork, SqlFuture, WriteWork};
use provenance_store::repository::{Repository, RepositoryIdentity};
use provenance_store::settings::Settings;
use provenance_store::state_store::CreateRequirementInput;
use provenance_store::storage::memory::{MemorySectionLock, MemoryStateStorage};
use serde_json::{json, Value};
use sqlite_database::MemorySqlite;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Notify;

/// What the next write of a [`Switched`] database does: it waits until the
/// test releases it, or it fails before its work runs.
#[derive(Debug, Default)]
struct Switch {
    hold: AtomicBool,
    fail: AtomicBool,
    entered: Notify,
    released: Notify,
}

impl Switch {
    /// The next write waits for [`Switch::release`] before its work runs.
    fn hold(&self) {
        self.hold.store(true, Ordering::SeqCst);
    }

    /// The next write fails, and its work does not run.
    fn fail(&self) {
        self.fail.store(true, Ordering::SeqCst);
    }

    /// Ends when a held write waits.
    async fn entered(&self) {
        self.entered.notified().await;
    }

    fn release(&self) {
        self.released.notify_one();
    }
}

/// The in-memory read database, whose writes the test can hold or fail.
#[derive(Debug)]
struct Switched {
    database: MemorySqlite,
    switch: Arc<Switch>,
}

impl ReadDatabase for Switched {
    fn read<'a>(&'a self, work: ReadWork<'a>) -> SqlFuture<'a, ()> {
        self.database.read(work)
    }

    fn write<'a>(&'a self, work: WriteWork<'a>) -> SqlFuture<'a, ()> {
        Box::pin(async move {
            if self.switch.hold.swap(false, Ordering::SeqCst) {
                self.switch.entered.notify_one();
                self.switch.released.notified().await;
            }
            if self.switch.fail.swap(false, Ordering::SeqCst) {
                return Err(anyhow::Error::msg("the test failed this write"));
            }
            self.database.write(work).await
        })
    }

    fn max_bound_values(&self) -> usize {
        self.database.max_bound_values()
    }

    fn cursor_key(&self) -> anyhow::Result<[u8; 32]> {
        self.database.cursor_key()
    }

    fn location(&self) -> String {
        self.database.location()
    }
}

/// A repository over memory storage whose read database is empty, and the
/// switch of its read database.
async fn repository() -> (tempfile::TempDir, Repository, Arc<Switch>) {
    let root = tempfile::tempdir().unwrap();
    let storage = Arc::new(MemoryStateStorage::loaded(
        initial_state(),
        Arc::new(Recorded::default()),
    ));
    let working_tree =
        camino::Utf8PathBuf::from_path_buf(dunce::canonicalize(root.path()).unwrap()).unwrap();
    let identity = RepositoryIdentity::new(working_tree, scope(), Settings::default());
    let switch = Arc::new(Switch::default());
    let database = Switched {
        database: MemorySqlite::new().await,
        switch: Arc::clone(&switch),
    };
    let repository = Repository::with_storage(
        identity,
        storage,
        Arc::new(MemorySectionLock::refusing()),
        Some(Arc::new(Other::new(database))),
    )
    .unwrap();
    (root, repository, switch)
}

/// The context of a call on the default scope of the target `memory`.
fn context() -> Value {
    json!({ "repository": "memory", "scope": "default" })
}

/// The context of a call that reads with `policy`.
fn with_policy(policy: &str) -> Value {
    let mut context = context();
    context["freshness"] = json!(policy);
    context
}

/// The answer of one read: the success, or the status and body of the
/// failure.
async fn read(repository: &Repository, operation: &'static str, request: Value) -> Value {
    read_in(repository, context(), operation, request).await
}

/// The answer of one read in `context`, through the in-process port of a
/// binding of `repository` under the target name `memory`.
async fn read_in(
    repository: &Repository,
    context: Value,
    operation: &'static str,
    request: Value,
) -> Value {
    let binding = LocalRepository::bind(repository.clone(), "memory").unwrap();
    let port = InProcessReadOperations::new(&binding);
    match port
        .run(operation, json!({ "context": context, "request": request }))
        .await
    {
        Ok(success) => success,
        Err(failure) => json!({
            "status": failure.status_code(),
            "error": failure.error,
        }),
    }
}

/// The stamp of an answer, with the instance id of the database removed:
/// each database has its own.
fn stamp(answer: &mut Value) -> Value {
    let mut stamp = answer
        .as_object_mut()
        .and_then(|answer| answer.remove("stamp"))
        .unwrap_or(Value::Null);
    if let Some(stamp) = stamp.as_object_mut() {
        stamp.remove("instance_id");
    }
    stamp
}

fn requirement(id: &str) -> CreateRequirementInput {
    CreateRequirementInput {
        scope_id: scope(),
        id: StableId::new(id).unwrap(),
        statement: "Share links have an owner".into(),
        description: None,
        status: RequirementStatus::Active,
        refines: Vec::new(),
        depends_on: Vec::new(),
        supersedes: Vec::new(),
        spawned_by: None,
        origin_thread: None,
        origin_message: None,
    }
}

fn ids(page: &Value) -> Vec<&str> {
    page["result"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["id"].as_str().unwrap())
        .collect()
}
