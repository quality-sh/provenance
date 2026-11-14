//! The answers and the state of one verification run that begins and
//! completes with no fault, over the memory adapter and a fixed clock.
//!
//! Characterization test: it keeps the answer of a call that succeeds while
//! the write order of the start of a run changes. It has no Rule, because no
//! Rule states these exact rows.

use crate::memory;

use memory::{begin_input, open_review, scope, verification_repository, FixedClock, Recorded};
use provenance_store::operations::{begin_verification, complete_verification};
use provenance_store::repository::Repository;
use provenance_store::state_store::CompleteVerificationInput;
use serde_json::{json, Value};
use std::sync::Arc;

const NOW: i64 = 1_767_225_600_000;

fn repository(root: &tempfile::TempDir) -> Repository {
    let clock = Arc::new(FixedClock::at_millis(NOW));
    let reviews = [open_review()];
    verification_repository(
        root.path(),
        &reviews,
        [],
        Arc::new(Recorded::default()),
        clock,
    )
    .1
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

    let begun = begin_verification(&repository, begin_input()).unwrap();
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
