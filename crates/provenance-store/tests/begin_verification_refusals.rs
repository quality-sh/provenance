//! The refusals of the start of a verification run that come before any
//! write, over the memory adapter: each gives its own kind, the storage
//! receives no write set, and the runs cache does not change.

#[path = "support/memory.rs"]
mod memory;

use memory::{
    begin_input, key, open_review, verification_repository, FixedClock, Recorded, REVIEWS,
};
use provenance_macros::verifies;
use provenance_store::operations::{
    begin_verification, complete_verification, verification_runs,
};
use provenance_store::repository::Repository;
use provenance_core::VerificationRun;
use provenance_store::state_store::{BeginVerificationInput, CompleteVerificationInput};
use provenance_store::storage::StateKey;
use provenance_store::write_error::WriteError;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;

const BINDINGS: &str = "scopes/default/verifications/binding.jsonl";
const RUNS: &str = ".provenance/cache/scopes/default/verification-runs.jsonl";
/// The `file` value of an answer for a line of the runs cache.
const RUNS_FILE: &str = "verification-runs.jsonl";
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
    let clock = Arc::new(FixedClock::at_millis(1_767_225_600_000));
    let reviews = [open_review()];
    let (_, repository) =
        verification_repository(root.path(), &reviews, extra, recorded.clone(), clock);
    Start {
        root,
        recorded,
        repository,
    }
}

impl Start {
    fn runs_cache(&self) -> PathBuf {
        self.root.path().join(RUNS)
    }

    /// The answer of a refused start, as [`Start::answer`] gives it.
    fn refusal(&self, input: BeginVerificationInput) -> (Value, u16, usize, Option<Vec<u8>>) {
        let error = begin_verification(&self.repository, input).unwrap_err();
        self.answer(error)
    }

    /// Like [`Start::refusal`], for a refused completion of a run.
    fn completion_refusal(&self) -> (Value, u16, usize, Option<Vec<u8>>) {
        let completion = CompleteVerificationInput {
            run: "verification_1767225600000".into(),
            status: "passed".into(),
            error: None,
        };
        let error = complete_verification(&self.repository, completion).unwrap_err();
        self.answer(error)
    }

    /// The runs that the list of runs gives, or its answer as
    /// [`Start::answer`] gives it.
    fn list(&self) -> Result<Vec<VerificationRun>, (Value, u16, usize, Option<Vec<u8>>)> {
        verification_runs(&self.repository, None).map_err(|error| self.answer(error))
    }

    /// The answer body of `error`, its status, the number of write sets
    /// that the storage received, and the bytes of the runs cache.
    fn answer(&self, error: anyhow::Error) -> (Value, u16, usize, Option<Vec<u8>>) {
        let error = WriteError(error);
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

// The Rule covers the answer. No Rule states the order of the refusals of
// a start.
#[test]
#[verifies("rule_state_parse_refusal_names_the_line", examples)]
fn a_bad_line_of_the_bindings_shard_is_refused_before_any_write() {
    let start = start(vec![(key(BINDINGS), BAD_LINE.to_vec())]);

    let refusal = start.refusal(begin_input());

    assert_eq!(refusal, (invalid_line(BINDINGS), 500, 0, None));
}

// The Rule covers the answer. No Rule states the order of the refusals of
// a start.
#[test]
#[verifies("rule_state_parse_refusal_names_the_line", examples)]
fn a_bad_line_of_the_reviews_shard_is_refused_before_any_write() {
    let start = start(vec![(key(REVIEWS), BAD_LINE.to_vec())]);

    let refusal = start.refusal(begin_input());

    assert_eq!(refusal, (invalid_line(REVIEWS), 500, 0, None));
}

/// A start whose runs cache holds `contents`.
fn start_with_runs_cache(contents: &[u8]) -> Start {
    let start = start(Vec::new());
    std::fs::create_dir_all(start.runs_cache().parent().unwrap()).unwrap();
    std::fs::write(start.runs_cache(), contents).unwrap();
    start
}

/// A start whose runs cache has a bad first line.
fn start_with_a_bad_runs_cache() -> Start {
    start_with_runs_cache(BAD_LINE)
}

// No Rule binding: `rule_state_parse_refusal_names_the_line` is for record
// files below the state directory, and the runs cache is a cache file. No
// Rule states the order of the refusals of a start. The answer is the
// answer of a completion for the same line, as the test below shows.
#[test]
fn a_bad_line_of_the_runs_cache_is_refused_before_any_write() {
    let start = start_with_a_bad_runs_cache();

    let refusal = start.refusal(begin_input());

    assert_eq!(
        refusal,
        (invalid_line(RUNS_FILE), 500, 0, Some(BAD_LINE.to_vec()))
    );
}

// Characterization test with no Rule binding: the answer of a completion for
// a bad line of the runs cache, which the start gives too.
#[test]
fn a_completion_gives_the_same_answer_for_a_bad_line_of_the_runs_cache() {
    let start = start_with_a_bad_runs_cache();

    let refusal = start.completion_refusal();

    assert_eq!(
        refusal,
        (invalid_line(RUNS_FILE), 500, 0, Some(BAD_LINE.to_vec()))
    );
}

// No Rule binding: `rule_state_parse_refusal_names_the_line` is for record
// files below the state directory, and the runs cache is a cache file.
#[test]
fn the_list_of_runs_gives_the_same_answer_for_a_bad_line_of_the_runs_cache() {
    let start = start_with_a_bad_runs_cache();

    let list = start.list();

    let answer = (invalid_line(RUNS_FILE), 500, 0, Some(BAD_LINE.to_vec()));
    assert_eq!(list, Err(answer));
}

// No Rule binding: no Rule states which stored run rows a build refuses.
#[test]
fn the_list_of_runs_refuses_a_run_row_that_the_start_and_the_completion_refuse() {
    let row = concat!(
        r#"{"scope_id":"default","id":"verification_1767225600000","#,
        r#""rule_id":"rule_expiry","method":"examples","declared_by":"cargo","#,
        r#""status":"running","started_at":1767225600000,"retired":false}"#,
        "\n"
    );
    let start = start_with_runs_cache(row.as_bytes());

    let answers = [
        start.list(),
        Err(start.refusal(begin_input())),
        Err(start.completion_refusal()),
    ];

    let message = "record verification_1767225600000 contains legacy field `retired`; \
                   canonical JSONL requires record-deletion migration 026 before this build \
                   can read it";
    let refusal = json!({
        "kind": "invalid_state_file", "file": RUNS_FILE, "line": 1, "message": message
    });
    let answer = Err((refusal, 500, 0, Some(row.as_bytes().to_vec())));
    assert_eq!(answers, [answer.clone(), answer.clone(), answer]);
}
