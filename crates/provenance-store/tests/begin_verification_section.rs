//! The start of a verification run and the publication section, over the
//! memory adapter. The clock of the repository runs a step of the test when
//! the start reads the time, which it does in the section, after the checks
//! and before the write set.

#[path = "support/memory.rs"]
mod memory;

use memory::{
    begin_input, key, open_review, scope, shard, verification_repository, FixedClock, Recorded,
};
use provenance_macros::verifies;
use provenance_store::operations::begin_verification;
use provenance_store::repository::Clock;
use provenance_store::storage::{Atomicity, StateStorage, WriteSet};
use provenance_store::write_error::WriteError;
use serde_json::json;
use std::sync::{Arc, Mutex};

const NOW: i64 = 1_767_225_600_000;

type Step = Box<dyn FnOnce() + Send>;

/// A clock that always reads `NOW` and runs its step at the first read.
struct SteppingClock {
    step: Mutex<Option<Step>>,
}

impl std::fmt::Debug for SteppingClock {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("SteppingClock")
    }
}

impl SteppingClock {
    fn new() -> Self {
        Self {
            step: Mutex::new(None),
        }
    }

    fn set(&self, step: impl FnOnce() + Send + 'static) {
        *self.step.lock().unwrap() = Some(Box::new(step));
    }
}

impl Clock for SteppingClock {
    fn now(&self) -> time::OffsetDateTime {
        let step = self.step.lock().unwrap().take();
        if let Some(step) = step {
            step();
        }
        FixedClock::at_millis(NOW).now()
    }
}

#[test]
#[verifies("rule_moved_base_refusal_is_state_changed", examples)]
fn a_state_change_outside_the_section_leaves_no_binding_and_no_run_row() {
    let root = tempfile::tempdir().unwrap();
    let clock = Arc::new(SteppingClock::new());
    let persistence = Arc::new(Recorded::default());
    let (storage, repository) =
        verification_repository(root.path(), &[open_review()], [], persistence, clock.clone());
    let requirement = json!({
        "scope_id": "default", "id": "req_expiry", "statement": "Share links expire",
        "description": "A change from outside the section", "status": "active"
    });
    let change = WriteSet::new(Atomicity::PerKey).put(
        key("scopes/default/requirements/req.jsonl"),
        shard(&[requirement]),
    );
    clock.set(move || storage.publish(change).unwrap());

    let error = WriteError(begin_verification(&repository, begin_input()).unwrap_err());

    let message = "The stored state changed after this write read it, so the write saved \
                   nothing. Read the records again, then send the write again.";
    assert_eq!(
        (serde_json::to_value(&error).unwrap(), error.status()),
        (json!({"kind": "state_changed", "message": message}), 409)
    );
    let store = repository.store();
    let reads = json!({
        "bindings": store.list_verification_bindings(&scope()).unwrap(),
        "runs": store.list_verification_runs(&scope()).unwrap(),
        "reviews": store.list_requirement_reviews(&scope()).unwrap(),
    });
    assert_eq!(
        reads,
        json!({"bindings": [], "runs": [], "reviews": [open_review()]})
    );
}
