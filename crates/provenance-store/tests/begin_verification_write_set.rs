//! The write set of the start of a verification run over the memory adapter,
//! where the persistence keeps each write set that it receives and can fail
//! each set that puts the reviews shard.

#[path = "support/memory.rs"]
mod memory;

use memory::{begin_input, open_review, scope, verification_repository, Failure, Refusing, REVIEWS};
use provenance_core::{VerificationBinding, VerificationRun};
use provenance_macros::verifies;
use provenance_store::operations::begin_verification;
use provenance_store::repository::{Repository, SystemClock};
use provenance_store::storage::{Atomicity, StateKey};
use provenance_store::write_error::WriteError;
use serde_json::{json, Value};
use std::sync::Arc;

const BINDINGS: &str = "scopes/default/verifications/binding.jsonl";

/// A memory repository with an open review of `rule_expiry`, over a
/// persistence that fails with `failure` each set that puts the reviews
/// shard.
struct Start {
    _root: tempfile::TempDir,
    persistence: Arc<Refusing>,
    repository: Repository,
}

fn start(failure: Option<Failure>) -> Start {
    let root = tempfile::tempdir().unwrap();
    let persistence = Arc::new(Refusing::new(|key| key == REVIEWS));
    persistence.fail(failure);
    let clock = Arc::new(SystemClock);
    let (_, repository) = verification_repository(root.path(), [], persistence.clone(), clock);
    Start {
        _root: root,
        persistence,
        repository,
    }
}

impl Start {
    /// The answer body of a failed start and its status.
    fn failure(&self) -> (Value, u16) {
        let error = WriteError(begin_verification(&self.repository, begin_input()).unwrap_err());
        (serde_json::to_value(&error).unwrap(), error.status())
    }

    /// The bindings, the runs and the reviews that the reads give.
    fn reads(&self) -> Value {
        let store = self.repository.store();
        json!({
            "bindings": store.list_verification_bindings(&scope()).unwrap(),
            "runs": store.list_verification_runs(&scope()).unwrap(),
            "reviews": store.list_requirement_reviews(&scope()).unwrap(),
        })
    }
}

/// The state before any start: no binding, no run and the open review.
fn untouched() -> Value {
    json!({"bindings": [], "runs": [], "reviews": [open_review()]})
}

#[test]
#[verifies("rule_write_sets_publish_through_the_storage_port", examples)]
fn a_start_publishes_the_binding_and_the_cleared_reviews_as_one_whole_set() {
    let start = start(None);

    begin_verification(&start.repository, begin_input()).unwrap();

    let sets: Vec<(Atomicity, Vec<String>)> = start
        .persistence
        .received()
        .iter()
        .map(|set| {
            let keys = set.puts().keys().map(StateKey::as_str).map(String::from);
            (set.atomicity(), keys.collect())
        })
        .collect();
    assert_eq!(
        sets,
        [(Atomicity::WholeSet, vec![REVIEWS.into(), BINDINGS.into()])]
    );
}

#[test]
#[verifies("rule_write_sets_publish_through_the_storage_port", examples)]
fn a_refused_set_leaves_no_binding_no_run_row_and_an_open_review() {
    let start = start(Some(Failure::Refused));

    let (answer, status) = start.failure();

    assert_eq!(
        (answer, status),
        (
            json!({"kind": "write_failed", "cause": "the persistence refused the set"}),
            500
        )
    );
    assert_eq!(start.reads(), untouched());
}

// No Rule binding: no Rule states that a start can follow a refused start.
#[test]
fn a_start_after_a_refused_set_clears_the_review() {
    let start = start(Some(Failure::Refused));
    start.failure();
    start.persistence.fail(None);

    let run = begin_verification(&start.repository, begin_input()).unwrap();

    let store = start.repository.store();
    let runs = store.list_verification_runs(&scope()).unwrap();
    assert_eq!(runs, std::slice::from_ref(&run));
    let reviews = store.list_requirement_reviews(&scope()).unwrap();
    assert_eq!(reviews[0].cleared_by_run.as_ref(), Some(&run.id));
}

// No Rule binding: no Rule states the run rows after a write set whose
// outcome is not known.
#[test]
fn a_set_with_an_unknown_outcome_gives_write_failed_and_adds_no_run_row() {
    let start = start(Some(Failure::Started));

    let (answer, status) = start.failure();

    let cause = "the persistence stopped while it kept the set";
    assert_eq!(
        (answer, status),
        (json!({"kind": "write_failed", "cause": cause}), 500)
    );
    let store = start.repository.store();
    let runs = store.list_verification_runs(&scope()).unwrap();
    assert_eq!(runs, [] as [VerificationRun; 0]);
    let bindings = store.list_verification_bindings(&scope()).unwrap();
    assert_eq!(bindings, [] as [VerificationBinding; 0]);
}
