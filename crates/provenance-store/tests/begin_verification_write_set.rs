//! The write set of the start of a verification run over the memory adapter,
//! where the persistence keeps each write set that it receives and can fail
//! each set that puts the reviews shard. A start publishes only the shards
//! that it changes.

#[path = "support/memory.rs"]
mod memory;

use memory::{
    begin_input, open_review, scope, verification_repository, Failure, FixedClock, Refusing,
    REVIEWS,
};
use provenance_core::{RequirementReview, VerificationBinding, VerificationRun};
use provenance_macros::verifies;
use provenance_store::operations::begin_verification;
use provenance_store::repository::Repository;
use provenance_store::storage::{Atomicity, StateKey};
use provenance_store::write_error::WriteError;
use serde_json::{json, Value};
use std::sync::Arc;

const BINDINGS: &str = "scopes/default/verifications/binding.jsonl";
const NOW: i64 = 1_767_225_600_000;

/// A memory repository at the fixed time `NOW`, over a persistence that
/// fails with `failure` each set that puts the reviews shard.
struct Start {
    _root: tempfile::TempDir,
    persistence: Arc<Refusing>,
    repository: Repository,
}

/// A start whose state holds an open review of `rule_expiry`.
fn start(failure: Option<Failure>) -> Start {
    start_with(failure, &[open_review()])
}

/// Like [`start`], but the reviews shard holds `reviews`, or the state has
/// no reviews shard when `reviews` is empty.
fn start_with(failure: Option<Failure>, reviews: &[Value]) -> Start {
    let root = tempfile::tempdir().unwrap();
    let persistence = Arc::new(Refusing::new(|key| key == REVIEWS));
    persistence.fail(failure);
    let clock = Arc::new(FixedClock::at_millis(NOW));
    let (_, repository) =
        verification_repository(root.path(), reviews, [], persistence.clone(), clock);
    Start {
        _root: root,
        persistence,
        repository,
    }
}

/// The atomicity and the put keys of each set that `persistence` received.
fn sets(persistence: &Refusing) -> Vec<(Atomicity, Vec<String>)> {
    let sets = persistence.received();
    sets.iter()
        .map(|set| {
            let keys = set.puts().keys().map(StateKey::as_str).map(String::from);
            (set.atomicity(), keys.collect())
        })
        .collect()
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
#[verifies("rule_store_sends_the_smallest_write_set", examples)]
fn a_start_publishes_the_binding_and_the_cleared_reviews_as_one_whole_set() {
    let start = start(None);

    begin_verification(&start.repository, begin_input()).unwrap();

    assert_eq!(
        sets(&start.persistence),
        [(Atomicity::WholeSet, vec![REVIEWS.into(), BINDINGS.into()])]
    );
}

// No Rule binding: no Rule states that a start with no change makes no
// write.
#[test]
fn a_repeated_start_sends_no_set_to_the_persistence() {
    let start = start(None);
    begin_verification(&start.repository, begin_input()).unwrap();

    begin_verification(&start.repository, begin_input()).unwrap();

    let first = (Atomicity::WholeSet, vec![REVIEWS.into(), BINDINGS.into()]);
    assert_eq!(sets(&start.persistence), [first]);
}

#[test]
#[verifies("rule_write_sets_publish_through_the_storage_port", examples)]
#[verifies("rule_store_sends_the_smallest_write_set", examples)]
fn a_start_in_a_scope_with_no_reviews_shard_sends_the_binding_as_a_one_key_set() {
    let start = start_with(None, &[]);

    begin_verification(&start.repository, begin_input()).unwrap();

    assert_eq!(
        sets(&start.persistence),
        [(Atomicity::PerKey, vec![BINDINGS.to_owned()])]
    );
}

#[test]
#[verifies("rule_write_sets_publish_through_the_storage_port", examples)]
#[verifies("rule_store_sends_the_smallest_write_set", examples)]
fn a_start_that_changes_only_the_binding_sends_a_one_key_set() {
    let start = start(None);
    begin_verification(&start.repository, begin_input()).unwrap();
    let mut other = begin_input();
    other.key = "expiry_again".into();

    begin_verification(&start.repository, other).unwrap();

    assert_eq!(
        sets(&start.persistence)[1..],
        [(Atomicity::PerKey, vec![BINDINGS.to_owned()])]
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

/// A review of `rule_expiry` raised at `changed_at`, with no clear.
fn review(id: &str, changed_at: i64) -> RequirementReview {
    let mut review: RequirementReview = serde_json::from_value(open_review()).unwrap();
    review.id = memory::id(id);
    review.changed_at = changed_at;
    review
}

/// `review` cleared by `run` at its start time.
fn cleared(mut review: RequirementReview, run: &VerificationRun) -> RequirementReview {
    review.cleared_at = Some(run.started_at);
    review.cleared_by_run = Some(run.id.clone());
    review
}

#[test]
#[verifies("rule_review_cleared_by_later_run", examples)]
fn a_run_clears_only_the_reviews_raised_before_it_and_keeps_their_reason() {
    let reviews = [review("review_expiry", 1), review("review_later", NOW + 1)];
    let start = start_with(
        None,
        &reviews.map(|review| serde_json::to_value(review).unwrap()),
    );

    let run = begin_verification(&start.repository, begin_input()).unwrap();

    let reviews = start
        .repository
        .store()
        .list_requirement_reviews(&scope())
        .unwrap();
    assert_eq!(
        reviews,
        [
            cleared(review("review_expiry", 1), &run),
            review("review_later", NOW + 1)
        ]
    );
    assert_eq!(
        (run.started_at, run.id.as_str()),
        (NOW, "verification_1767225600000")
    );
}

// No Rule binding: no Rule states the state after a retry of a refused set.
#[test]
fn a_start_after_a_refused_set_adds_its_run_and_clears_the_review() {
    let start = start(Some(Failure::Refused));
    start.failure();
    start.persistence.fail(None);

    let run = begin_verification(&start.repository, begin_input()).unwrap();

    let store = start.repository.store();
    let runs = store.list_verification_runs(&scope()).unwrap();
    assert_eq!(runs, std::slice::from_ref(&run));
    let reviews = store.list_requirement_reviews(&scope()).unwrap();
    assert_eq!(reviews, [cleared(review("review_expiry", 1), &run)]);
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
