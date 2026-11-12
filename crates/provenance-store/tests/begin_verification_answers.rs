//! The answers and the state of one verification run that begins and
//! completes with no fault, over the memory adapter and a fixed clock.
//!
//! Characterization test: it keeps the answer of a call that succeeds while
//! the write order of the start of a run changes. It has no Rule, because no
//! Rule states these exact rows.

#[path = "support/memory.rs"]
mod memory;

use memory::{initial_state, key, memory_repository_at, scope, shard, Recorded};
use provenance_store::operations::{begin_verification, complete_verification};
use provenance_store::repository::{Clock, Repository};
use provenance_store::state_store::{BeginVerificationInput, CompleteVerificationInput};
use provenance_store::storage::StateView;
use serde_json::{json, Value};
use std::sync::Arc;

const NOW: i64 = 1_767_225_600_000;
const REVIEWS: &str = "scopes/default/requirements/review.jsonl";

#[derive(Debug)]
struct Fixed;

impl Clock for Fixed {
    fn now(&self) -> time::OffsetDateTime {
        time::OffsetDateTime::from_unix_timestamp_nanos(i128::from(NOW) * 1_000_000).unwrap()
    }
}

fn open_review() -> Value {
    json!({
        "scope_id": "default", "id": "review_expiry", "rule_id": "rule_expiry",
        "requirement_id": "req_expiry", "field": "statement",
        "before": "Links expire", "after": "Share links expire", "changed_at": 1
    })
}

fn repository(root: &tempfile::TempDir) -> Repository {
    std::fs::write(root.path().join("evidence.rs"), "fn expiry() {}\n").unwrap();
    let mut entries = initial_state().into_entries();
    entries.insert(key(REVIEWS), shard(&[open_review()]));
    let persistence = Arc::new(Recorded::default());
    let clock = Arc::new(Fixed);
    memory_repository_at(root.path(), StateView::new(entries), persistence, clock).1
}

fn input() -> BeginVerificationInput {
    serde_json::from_value(json!({
        "rule": "rule_expiry", "key": "expiry", "method": "examples",
        "declared_by": "suite", "file": "evidence.rs", "symbol": "expiry"
    }))
    .unwrap()
}

/// The bindings, the runs and the reviews that the reads give.
fn reads(repository: &Repository) -> Value {
    let store = repository.store();
    json!({
        "bindings": store.list_verification_bindings(&scope()).unwrap(),
        "runs": store.list_verification_runs(&scope()).unwrap(),
        "reviews": store.list_requirement_reviews(&scope()).unwrap(),
    })
}

fn run(status: &str, completed_at: Option<i64>) -> Value {
    let mut run = json!({
        "scope_id": "default", "id": format!("verification_{NOW}"),
        "binding_id": "verification_binding_c78c01aaf5cf40061fbf",
        "rule_id": "rule_expiry", "method": "examples", "declared_by": "suite",
        "file": "evidence.rs", "symbol": "expiry", "status": status, "started_at": NOW
    });
    if let Some(completed_at) = completed_at {
        run["completed_at"] = json!(completed_at);
    }
    run
}

fn binding() -> Value {
    json!({
        "scope_id": "default", "id": "verification_binding_c78c01aaf5cf40061fbf",
        "rule_id": "rule_expiry", "key": "expiry", "method": "examples",
        "declared_by": "suite", "file": "evidence.rs", "symbol": "expiry"
    })
}

fn cleared_review() -> Value {
    let mut review = open_review();
    review["cleared_at"] = json!(NOW);
    review["cleared_by_run"] = json!(format!("verification_{NOW}"));
    review
}

#[test]
fn a_run_that_begins_and_completes_keeps_its_answers_and_rows() {
    let root = tempfile::tempdir().unwrap();
    let repository = repository(&root);

    let begun = begin_verification(&repository, input()).unwrap();
    assert_eq!(serde_json::to_value(&begun).unwrap(), run("running", None));
    assert_eq!(
        reads(&repository),
        json!({
            "bindings": [binding()],
            "runs": [run("running", None)],
            "reviews": [cleared_review()],
        })
    );

    let completion = CompleteVerificationInput {
        run: begun.id.as_str().into(),
        status: "passed".into(),
        error: None,
    };
    let completed = complete_verification(&repository, completion).unwrap();
    assert_eq!(
        serde_json::to_value(&completed).unwrap(),
        run("passed", Some(NOW))
    );
    assert_eq!(
        reads(&repository),
        json!({
            "bindings": [binding()],
            "runs": [run("passed", Some(NOW))],
            "reviews": [cleared_review()],
        })
    );
}
