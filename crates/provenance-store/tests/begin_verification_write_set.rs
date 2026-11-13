//! The start of a verification run over the memory adapter, where the
//! persistence keeps each write set that it receives and can refuse each set
//! that puts the reviews shard.

#[path = "support/memory.rs"]
mod memory;

use memory::{initial_state, key, memory_repository_over, scope, shard};
use provenance_core::{VerificationBinding, VerificationRun};
use provenance_macros::verifies;
use provenance_store::operations::begin_verification;
use provenance_store::repository::Repository;
use provenance_store::state_store::BeginVerificationInput;
use provenance_store::storage::{Atomicity, PublishFailure, StatePersistence, WriteSet};
use provenance_store::storage::{StateKey, StateView};
use provenance_store::write_error::WriteError;
use serde_json::json;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

const BINDINGS: &str = "scopes/default/verifications/binding.jsonl";
const REVIEWS: &str = "scopes/default/requirements/review.jsonl";

/// Keeps each set that it receives. Refuses each set that puts the reviews
/// shard while `refuse` is set.
#[derive(Debug, Default)]
struct Sets {
    refuse: AtomicBool,
    received: Mutex<Vec<WriteSet>>,
}

impl StatePersistence for Sets {
    fn persist(&self, writes: &WriteSet) -> Result<(), PublishFailure> {
        self.received.lock().unwrap().push(writes.clone());
        let reviews = writes.puts().keys().any(|key| key.as_str() == REVIEWS);
        if reviews && self.refuse.load(Ordering::SeqCst) {
            return Err(PublishFailure::Refused(anyhow::anyhow!(
                "the persistence refused the reviews shard"
            )));
        }
        Ok(())
    }
}

impl Sets {
    /// The atomicity and the put keys of each set that the persistence
    /// received, from the first.
    fn received(&self) -> Vec<(Atomicity, Vec<String>)> {
        let sets = self.received.lock().unwrap();
        sets.iter()
            .map(|set| {
                let keys = set.puts().keys().map(StateKey::as_str).map(String::from);
                (set.atomicity(), keys.collect())
            })
            .collect()
    }
}

fn open_review() -> serde_json::Value {
    json!({
        "scope_id": "default", "id": "review_expiry", "rule_id": "rule_expiry",
        "requirement_id": "req_expiry", "field": "statement",
        "before": "Links expire", "after": "Share links expire", "changed_at": 1
    })
}

/// A memory repository whose state holds an open review of `rule_expiry`.
fn repository(root: &tempfile::TempDir, sets: Arc<Sets>) -> Repository {
    std::fs::write(root.path().join("evidence.rs"), "fn expiry() {}\n").unwrap();
    let mut entries = initial_state().into_entries();
    entries.insert(key(REVIEWS), shard(&[open_review()]));
    memory_repository_over(root.path(), StateView::new(entries), sets).1
}

fn input() -> BeginVerificationInput {
    serde_json::from_value(json!({
        "rule": "rule_expiry", "key": "expiry", "method": "examples",
        "declared_by": "suite", "file": "evidence.rs"
    }))
    .unwrap()
}

#[test]
#[verifies("rule_write_sets_publish_through_the_storage_port", examples)]
fn a_refused_begin_set_leaves_no_binding_and_no_run_row() {
    let root = tempfile::tempdir().unwrap();
    let sets = Arc::new(Sets::default());
    let repository = repository(&root, sets.clone());
    let store = repository.store();
    sets.refuse.store(true, Ordering::SeqCst);

    let error = WriteError(begin_verification(&repository, input()).unwrap_err());

    assert_eq!(
        serde_json::to_value(&error).unwrap(),
        json!({"kind": "write_failed", "cause": "the persistence refused the reviews shard"})
    );
    assert_eq!(error.status(), 500);
    assert_eq!(
        sets.received(),
        [(Atomicity::PerKey, vec![REVIEWS.into(), BINDINGS.into()])],
        "one attempt"
    );
    let bindings = store.list_verification_bindings(&scope()).unwrap();
    assert_eq!(bindings, [] as [VerificationBinding; 0]);
    let runs = store.list_verification_runs(&scope()).unwrap();
    assert_eq!(runs, [] as [VerificationRun; 0]);
    let reviews = store.list_requirement_reviews(&scope()).unwrap();
    assert_eq!(
        serde_json::to_value(&reviews).unwrap(),
        json!([open_review()])
    );

    sets.refuse.store(false, Ordering::SeqCst);
    let run = begin_verification(&repository, input()).unwrap();
    let runs = store.list_verification_runs(&scope()).unwrap();
    assert_eq!(runs, std::slice::from_ref(&run));
    let reviews = store.list_requirement_reviews(&scope()).unwrap();
    assert_eq!(reviews[0].cleared_by_run.as_ref(), Some(&run.id));
}
