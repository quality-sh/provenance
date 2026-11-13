//! The refusals of the start of a verification run that come before any
//! write, over the memory adapter: each gives its own kind, the storage
//! receives no write set, and the runs cache does not change.

#[path = "support/memory.rs"]
mod memory;

use memory::{begin_input, key, verification_repository, Recorded, REVIEWS};
use provenance_store::operations::begin_verification;
use provenance_store::repository::{Repository, SystemClock};
use provenance_store::state_store::BeginVerificationInput;
use provenance_store::storage::StateKey;
use provenance_store::write_error::WriteError;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;

const BINDINGS: &str = "scopes/default/verifications/binding.jsonl";
const BAD_LINE: &[u8] = b"{not json\n";

/// A memory repository with an open review of `rule_expiry`, whose state
/// also holds `extra`.
struct Start {
    root: tempfile::TempDir,
    recorded: Arc<Recorded>,
    repository: Repository,
}

fn start(extra: Vec<(StateKey, Vec<u8>)>) -> Start {
    let root = tempfile::tempdir().unwrap();
    let recorded = Arc::new(Recorded::default());
    let clock = Arc::new(SystemClock);
    let (_, repository) = verification_repository(root.path(), extra, recorded.clone(), clock);
    Start {
        root,
        recorded,
        repository,
    }
}

impl Start {
    fn runs_cache(&self) -> PathBuf {
        let runs = ".provenance/cache/scopes/default/verification-runs.jsonl";
        self.root.path().join(runs)
    }

    /// The answer body of a refused start, its status, the number of write
    /// sets that the storage received, and the bytes of the runs cache.
    fn refusal(&self, input: BeginVerificationInput) -> (Value, u16, usize, Option<Vec<u8>>) {
        let error = WriteError(begin_verification(&self.repository, input).unwrap_err());
        let answer = serde_json::to_value(&error).unwrap();
        let sets = self.recorded.0.lock().unwrap().len();
        (
            answer,
            error.status(),
            sets,
            std::fs::read(self.runs_cache()).ok(),
        )
    }
}

fn invalid_line(file: &str) -> Value {
    json!({
        "kind": "invalid_state_file", "file": file, "line": 1,
        "message": "key must be a string"
    })
}

// No Rule binding: no Rule states the order of the refusals of a start.
#[test]
fn a_rule_that_does_not_exist_is_refused_before_any_write() {
    let start = start(Vec::new());
    let mut input = begin_input();
    input.rule = Some("rule_absent".into());

    let refusal = start.refusal(input);

    let answer = json!({"kind": "invalid_verification_target"});
    assert_eq!(refusal, (answer, 400, 0, None));
}

// No Rule binding: no Rule states the order of the refusals of a start.
#[test]
fn a_bad_line_of_the_bindings_shard_is_refused_before_any_write() {
    let start = start(vec![(key(BINDINGS), BAD_LINE.to_vec())]);

    let refusal = start.refusal(begin_input());

    assert_eq!(refusal, (invalid_line(BINDINGS), 500, 0, None));
}

// No Rule binding: no Rule states the order of the refusals of a start.
#[test]
fn a_bad_line_of_the_reviews_shard_is_refused_before_any_write() {
    let start = start(vec![(key(REVIEWS), BAD_LINE.to_vec())]);

    let refusal = start.refusal(begin_input());

    assert_eq!(refusal, (invalid_line(REVIEWS), 500, 0, None));
}

// No Rule binding: no Rule states the order of the refusals of a start.
#[test]
fn a_bad_line_of_the_runs_cache_is_refused_before_any_write() {
    let start = start(Vec::new());
    std::fs::create_dir_all(start.runs_cache().parent().unwrap()).unwrap();
    std::fs::write(start.runs_cache(), BAD_LINE).unwrap();

    let refusal = start.refusal(begin_input());

    let cache = "verification-runs.jsonl";
    assert_eq!(
        refusal,
        (invalid_line(cache), 500, 0, Some(BAD_LINE.to_vec()))
    );
}
