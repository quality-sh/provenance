//! The start of a verification run on the local disk adapter, when the
//! process cannot write one directory of the repository. Unix only: the
//! tests take the write permission from a directory with its mode.
#![cfg(unix)]

#[path = "support/memory.rs"]
mod memory;

use memory::{initial_state, key, scope, shard};
use provenance_core::{VerificationBinding, VerificationRun};
use provenance_store::operations::begin_verification;
use provenance_store::repository::Repository;
use provenance_store::state_store::BeginVerificationInput;
use provenance_store::write_error::WriteError;
use serde_json::{json, Value};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

const REVIEWS: &str = "scopes/default/requirements/review.jsonl";

/// Takes the write permission from a directory, and gives it back on drop.
struct ReadOnly(PathBuf);

impl ReadOnly {
    fn new(path: PathBuf) -> Self {
        std::fs::create_dir_all(&path).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o555)).unwrap();
        let guard = Self(path);
        assert!(
            std::fs::write(guard.0.join("probe"), b"x").is_err(),
            "the test user can write a read-only directory"
        );
        guard
    }
}

impl Drop for ReadOnly {
    fn drop(&mut self) {
        let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o755));
    }
}

fn open_review() -> Value {
    json!({
        "scope_id": "default", "id": "review_expiry", "rule_id": "rule_expiry",
        "requirement_id": "req_expiry", "field": "statement",
        "before": "Links expire", "after": "Share links expire", "changed_at": 1
    })
}

/// A repository on the local disk whose state holds one active Rule and an
/// open review of it.
fn repository(root: &Path) -> Repository {
    std::fs::write(root.join("evidence.rs"), "fn expiry() {}\n").unwrap();
    let state = root.join(".provenance/state");
    let mut entries = initial_state().into_entries();
    entries.insert(key(REVIEWS), shard(&[open_review()]));
    for (key, bytes) in entries {
        let path = state.join(key.as_str());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }
    Repository::open(root, "default").unwrap()
}

fn input() -> BeginVerificationInput {
    serde_json::from_value(json!({
        "rule": "rule_expiry", "key": "expiry", "method": "examples",
        "declared_by": "suite", "file": "evidence.rs"
    }))
    .unwrap()
}

/// The answer body of a failed start and its status.
fn failure(repository: &Repository) -> (Value, u16) {
    let error = WriteError(begin_verification(repository, input()).unwrap_err());
    (serde_json::to_value(&error).unwrap(), error.status())
}

fn assert_permission_denied(answer: &Value) {
    assert_eq!(answer["kind"], "write_failed", "{answer}");
    let cause = answer["cause"].as_str().unwrap();
    assert!(
        cause.starts_with("Permission denied (os error 13) at path \"") && cause.contains(".tmp"),
        "{answer}"
    );
}

/// This test shows more than a Rule states: no Rule states the state after a
/// failed write of the run row.
#[test]
fn a_run_row_that_cannot_be_written_leaves_no_binding() {
    let root = tempfile::tempdir().unwrap();
    let repository = repository(root.path());
    let cache = ReadOnly::new(root.path().join(".provenance/cache/scopes/default"));

    let (answer, status) = failure(&repository);

    assert_permission_denied(&answer);
    assert_eq!(status, 500);
    let store = repository.store();
    let bindings = store.list_verification_bindings(&scope()).unwrap();
    assert_eq!(bindings, [] as [VerificationBinding; 0]);
    let runs = store.list_verification_runs(&scope()).unwrap();
    assert_eq!(runs, [] as [VerificationRun; 0]);
    let reviews = store.list_requirement_reviews(&scope()).unwrap();
    assert_eq!(serde_json::to_value(&reviews).unwrap(), json!([open_review()]));

    drop(cache);
    let run = begin_verification(&repository, input()).unwrap();
    assert_eq!(store.list_verification_runs(&scope()).unwrap(), [run]);
    assert_eq!(store.list_verification_bindings(&scope()).unwrap().len(), 1);
}

/// The local adapter writes the keys of a per-key set one at a time, in key
/// order, so the reviews key comes before the binding key. A refused reviews
/// key thus stops the set after landing began, and the run row stays. This
/// test shows more than a Rule states: no Rule states this state.
#[test]
fn a_refused_reviews_key_on_local_disk_leaves_a_running_row() {
    let root = tempfile::tempdir().unwrap();
    let repository = repository(root.path());
    let reviews =
        ReadOnly::new(root.path().join(".provenance/state/scopes/default/requirements"));

    let (answer, status) = failure(&repository);

    assert_permission_denied(&answer);
    assert_eq!(status, 500);
    let store = repository.store();
    let bindings = store.list_verification_bindings(&scope()).unwrap();
    assert_eq!(bindings, [] as [VerificationBinding; 0]);
    let runs = store.list_verification_runs(&scope()).unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].status, provenance_core::VerificationRunStatus::Running);
    let open = store.list_requirement_reviews(&scope()).unwrap();
    assert_eq!(serde_json::to_value(&open).unwrap(), json!([open_review()]));

    drop(reviews);
    let run = begin_verification(&repository, input()).unwrap();
    assert_eq!(store.list_verification_runs(&scope()).unwrap().len(), 2);
    let cleared = store.list_requirement_reviews(&scope()).unwrap();
    assert_eq!(cleared[0].cleared_by_run.as_ref(), Some(&run.id));
}
