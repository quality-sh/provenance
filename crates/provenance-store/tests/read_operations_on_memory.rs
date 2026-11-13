//! The read operations over memory storage, with a read database that only
//! the embedder catches up.
//!
//! Implementation aid: no Rule in the graph covers a read without `host`.
//! These tests show what an embedder gets. The tests run with and without
//! `host`. Without it a read takes no publication guard and never writes
//! the read database: it answers from the database as the last
//! `catch_up_in_publish` left it, and its stamp says `annotate_only`. With
//! `host` a read catches the database up itself, as before.
//!
//! The read database is an `SQLite` database in memory behind the
//! contract's wrapper, so the reads reach it through the port only. The
//! section lock refuses where another lock would wait, so a test that
//! passes took no lock that was held.

#[path = "support/memory.rs"]
mod memory;
#[path = "support/sqlite_database.rs"]
mod sqlite_database;

use memory::{initial_state, scope, Recorded};
use provenance_core::{RequirementStatus, StableId};
use provenance_store::cache::catch_up_in_publish;
use provenance_store::operations::read_operations::{InProcessReadOperations, ReadOperations};
use provenance_store::read_database::contract::Other;
use provenance_store::repository::{Repository, RepositoryIdentity};
use provenance_store::settings::Settings;
use provenance_store::state_store::CreateRequirementInput;
use provenance_store::storage::memory::MemoryStateStorage;
use provenance_store::storage::SectionLock;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlite_database::MemorySqlite;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// The stamp policy of a read that asks for no freshness step.
#[cfg(feature = "host")]
const DEFAULT_POLICY: &str = "catch_up";
#[cfg(not(feature = "host"))]
const DEFAULT_POLICY: &str = "annotate_only";

/// A section lock that refuses when it is held, where another lock waits.
#[derive(Debug, Default)]
struct RefusesWhenHeld(Arc<AtomicBool>);

struct Held(Arc<AtomicBool>);

impl Drop for Held {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

impl SectionLock for RefusesWhenHeld {
    fn acquire(&self) -> anyhow::Result<Box<dyn Send>> {
        anyhow::ensure!(
            !self.0.swap(true, Ordering::AcqRel),
            "the section is held, so this acquire would wait"
        );
        Ok(Box::new(Held(Arc::clone(&self.0))))
    }
}

/// A repository over memory storage whose read database is empty.
async fn repository() -> (tempfile::TempDir, Repository) {
    let root = tempfile::tempdir().unwrap();
    let storage = Arc::new(MemoryStateStorage::loaded(
        initial_state(),
        Arc::new(Recorded::default()),
    ));
    let working_tree =
        camino::Utf8PathBuf::from_path_buf(dunce::canonicalize(root.path()).unwrap()).unwrap();
    let identity = RepositoryIdentity::new(working_tree, scope(), Settings::default());
    let repository = Repository::with_storage(
        identity,
        storage,
        Arc::new(RefusesWhenHeld::default()),
        Some(Arc::new(Other(MemorySqlite::new().await))),
    )
    .unwrap();
    (root, repository)
}

/// The context of a call on the default scope of the target `memory`.
fn context() -> Value {
    json!({ "repository": "memory", "scope": "default" })
}

/// The answer of one read: the success, or the status and body of the
/// failure.
async fn read(repository: &Repository, operation: &'static str, request: Value) -> Value {
    read_in(repository, context(), operation, request).await
}

/// The answer of one read in `context`.
async fn read_in(
    repository: &Repository,
    context: Value,
    operation: &'static str,
    request: Value,
) -> Value {
    let port = InProcessReadOperations::new(repository.clone(), "memory").unwrap();
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

/// One call of each read operation.
fn reads() -> Vec<(&'static str, Value)> {
    let parent = json!({ "node_type": "requirement", "node_id": "req_expiry" });
    vec![
        ("page-sources", json!({})),
        ("page-requirements", json!({ "limit": 1 })),
        ("page-resolutions", json!({})),
        ("page-rules", json!({})),
        ("page-topics", json!({})),
        ("page-questions", json!({})),
        (
            "get-reviewed-resource",
            json!({ "record_kind": "rule", "id": "rule_expiry" }),
        ),
        ("read-document", json!({ "id": "req_expiry" })),
        ("review-discussions", json!({ "parent": parent })),
        (
            "review-discussion-messages",
            json!({
                "parent": parent,
                "selector": { "kind": "discussion", "discussion_id": "discussion_none" },
            }),
        ),
    ]
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

/// The answers of the read operations, after the embedder's catch-up, are
/// the same with and without `host`; only the stamp policy differs.
#[tokio::test]
async fn the_reads_answer_alike_with_and_without_host() {
    let (_root, repository) = repository().await;
    let report = catch_up_in_publish(&repository).await.unwrap();
    assert!(report.rebuilt, "the first pass builds the projection");

    let mut answers = Vec::new();
    for (operation, request) in reads() {
        let mut answer = read(&repository, operation, request).await;
        let stamp = stamp(&mut answer);
        if !stamp.is_null() {
            assert_eq!(stamp["policy"], DEFAULT_POLICY, "{operation}: {stamp}");
            assert_eq!(stamp["serial"], report.serial, "{operation}: {stamp}");
        }
        answers.push(json!({ "operation": operation, "answer": answer }));
    }
    let answers = serde_json::to_string_pretty(&answers).unwrap();
    assert_eq!(
        hex::encode(&Sha256::digest(answers.as_bytes())[..8]),
        "e6df0bb37c1524f5",
        "the answers moved:\n{answers}"
    );
}

/// Without `host` a read before any catch-up refuses as an `annotate_only`
/// read refuses a database that holds no current schema: `schema_behind`,
/// 409. With `host` the read builds the projection itself.
#[tokio::test]
async fn a_read_before_any_catch_up_answers_as_its_build_says() {
    let (_root, repository) = repository().await;
    let mut answer = read(&repository, "page-requirements", json!({})).await;
    let stamp = stamp(&mut answer);
    #[cfg(not(feature = "host"))]
    {
        assert!(stamp.is_null(), "{answer}");
        assert_eq!(answer["status"], 409, "{answer}");
        assert_eq!(answer["error"]["kind"], "schema_behind", "{answer}");
    }
    #[cfg(feature = "host")]
    {
        assert_eq!(stamp["policy"], "catch_up", "{stamp}");
        assert_eq!(ids(&answer), ["req_expiry"], "{answer}");
    }
}

/// A publish that the embedder follows with its catch-up shows in the next
/// read. Without `host` a read between the two answers at the serial of the
/// last catch-up, and its stamp says that no catch-up ran.
#[tokio::test]
async fn a_publish_then_the_catch_up_shows_in_the_next_read() {
    let (_root, repository) = repository().await;
    let first = catch_up_in_publish(&repository).await.unwrap();

    repository
        .store()
        .create_requirement(requirement("req_owner"))
        .unwrap();
    let mut between = read(&repository, "page-requirements", json!({})).await;
    let stamp_between = stamp(&mut between);
    assert_eq!(stamp_between["policy"], DEFAULT_POLICY, "{stamp_between}");
    #[cfg(not(feature = "host"))]
    {
        assert_eq!(stamp_between["serial"], first.serial, "{stamp_between}");
        assert_eq!(ids(&between), ["req_expiry"], "{between}");
    }
    #[cfg(feature = "host")]
    {
        assert_eq!(stamp_between["serial"], first.serial + 1, "{stamp_between}");
        assert_eq!(ids(&between), ["req_expiry", "req_owner"], "{between}");
    }

    let second = catch_up_in_publish(&repository).await.unwrap();
    assert_eq!(second.serial, first.serial + 1);
    let mut after = read(&repository, "page-requirements", json!({})).await;
    let stamp_after = stamp(&mut after);
    assert_eq!(stamp_after["serial"], second.serial, "{stamp_after}");
    assert_eq!(ids(&after), ["req_expiry", "req_owner"], "{after}");
}

/// The in-process port runs the calls of the target name that it is given,
/// and refuses a call that names another target, as the router does.
#[tokio::test]
async fn a_call_that_names_another_target_refuses() {
    let (_root, repository) = repository().await;
    catch_up_in_publish(&repository).await.unwrap();
    let other = json!({ "repository": "other", "scope": "default" });
    let answer = read_in(&repository, other, "page-requirements", json!({})).await;
    assert_eq!(answer["status"], 404, "{answer}");
    assert_eq!(answer["error"]["kind"], "unknown_target", "{answer}");
}

/// A `refuse_stale` read over memory storage hashes the storage: it answers
/// after the embedder's catch-up, and refuses after a publish that no
/// catch-up followed, with the moved unit.
#[tokio::test]
async fn a_refuse_stale_read_hashes_the_storage() {
    let (_root, repository) = repository().await;
    let caught_up = catch_up_in_publish(&repository).await.unwrap();
    let mut refusing = context();
    refusing["freshness"] = json!("refuse_stale");

    let mut fresh = read_in(
        &repository,
        refusing.clone(),
        "page-requirements",
        json!({}),
    )
    .await;
    let stamp = stamp(&mut fresh);
    assert_eq!(stamp["policy"], "refuse_stale", "{fresh}");
    assert_eq!(stamp["serial"], caught_up.serial, "{stamp}");
    assert_eq!(ids(&fresh), ["req_expiry"], "{fresh}");

    repository
        .store()
        .create_requirement(requirement("req_owner"))
        .unwrap();
    let stale = read_in(&repository, refusing, "page-requirements", json!({})).await;
    assert_eq!(stale["status"], 409, "{stale}");
    assert_eq!(stale["error"]["kind"], "stale", "{stale}");
    assert_eq!(stale["error"]["serial"], caught_up.serial, "{stale}");
    let moved: Vec<&Value> = stale["error"]["moved"].as_array().unwrap().iter().collect();
    assert!(
        moved.iter().any(|unit| unit["unit"] == "scope:default"),
        "{stale}"
    );
}

/// Without `host` an acquire of a held memory section refuses at once: no
/// thread waits for a section.
#[cfg(not(feature = "host"))]
#[test]
fn a_held_memory_section_refuses_without_host() {
    let lock = provenance_store::storage::memory::MemorySectionLock::default();
    let held = lock.acquire().unwrap();
    let other = lock.clone();
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || sender.send(other.acquire().is_err()).unwrap());
    let refused = receiver
        .recv_timeout(std::time::Duration::from_secs(10))
        .expect("the second acquire waits for the held section");
    assert!(refused, "the second acquire took the held section");
    drop(held);
    assert!(lock.acquire().is_ok());
}
