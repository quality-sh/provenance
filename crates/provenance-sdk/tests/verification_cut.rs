//! A failure text that does not fit the run row is stored cut, with a mark,
//! and `verify` gives the complete text to its caller.

mod support;

use provenance_core::{ScopeId, VerificationRun, VerificationRunStatus};
use provenance_macros::verifies;
use provenance_sdk::{operations, verify};
use provenance_store::layout::ProvenanceLayout;
use support::{recorded_runs, seeded_rule};

/// The bytes of one run row that the reads of the runs give. The store owns
/// the number in `read_budget::resource_record_bytes`.
const BUDGET: usize = 1_048_576 - 16_384;

fn repository() -> (tempfile::TempDir, std::sync::MutexGuard<'static, ()>) {
    support::repository("verification_cut.rs", include_str!("verification_cut.rs"))
}

/// The bytes of the failed row of the running run with an empty text. The
/// callback reads the run that `verify` started.
fn row_without_text(root: &std::path::Path) -> usize {
    let mut run = recorded_runs(root)
        .into_iter()
        .find(|run| run.status == VerificationRunStatus::Running)
        .unwrap();
    run.status = VerificationRunStatus::Failed;
    run.completed_at = Some(run.started_at);
    run.error = Some(String::new());
    serde_json::to_vec(&run).unwrap().len()
}

/// A failure text that makes the failed row of the running run `row` bytes
/// long.
fn text_for_row(root: &std::path::Path, row: usize) -> String {
    "x".repeat(row - row_without_text(root))
}

/// The newest stored run.
fn newest_run(root: &std::path::Path) -> VerificationRun {
    let mut runs = recorded_runs(root);
    runs.sort_by(|left, right| {
        (left.started_at, left.id.as_str()).cmp(&(right.started_at, right.id.as_str()))
    });
    runs.pop().unwrap()
}

/// The runs cache of the repository at `root`.
fn runs_file(root: &std::path::Path) -> camino::Utf8PathBuf {
    let root = camino::Utf8Path::from_path(root).unwrap();
    let layout = ProvenanceLayout::new(root.to_owned());
    layout.verification_runs_path(&ScopeId::new("default").unwrap())
}

/// The newest run that the evidence read of `rule` gives.
fn evidence_run(root: &std::path::Path, rule: &str) -> Option<VerificationRun> {
    let query = provenance_core::protocol::EvidenceQuery {
        protocol_version: Some(provenance_sdk::SDK_PROTOCOL_VERSION),
        rule: rule.to_owned(),
        base: None,
        head: None,
        limit: 50,
    };
    let evidence = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(operations::queries::evidence(
            &support::open(root),
            operations::read_policy::ReadPolicy::default(),
            query,
        ))
        .unwrap();
    evidence.result.latest_verification_run
}

/// Asserts that `run` failed with a row of `row` bytes, and that its stored
/// text is the start of `text`, cut at a character boundary, with a mark that
/// gives the byte count of the stored part and of `text`.
fn assert_stored_cut(run: &VerificationRun, text: &str, row: usize) {
    assert_eq!(run.status, VerificationRunStatus::Failed);
    assert_eq!(serde_json::to_vec(run).unwrap().len(), row);
    let stored = run.error.as_deref().unwrap();
    let (kept, mark) = stored.split_at(stored.rfind("… [cut: ").unwrap());
    assert!(text.is_char_boundary(kept.len()));
    assert!(text.starts_with(kept));
    assert!(kept.len() < text.len());
    assert_eq!(
        mark,
        format!("… [cut: {} of {} bytes]", kept.len(), text.len())
    );
}

/// Asserts that `actual` is `expected`. A failure gives the byte counts and
/// not the texts, which are about 1 MB.
fn assert_same_text(actual: Option<&str>, expected: &str) {
    assert!(
        actual == Some(expected),
        "a text of {:?} bytes and not the expected text of {} bytes",
        actual.map(str::len),
        expected.len()
    );
}

/// Asserts that the evidence read of `rule` gives `run` as its newest run.
fn assert_evidence_gives(root: &std::path::Path, rule: &str, run: &VerificationRun) {
    let given = evidence_run(root, rule);
    assert!(
        given.as_ref() == Some(run),
        "the evidence read gives {:?} and not the run {}",
        given.as_ref().map(|given| given.id.as_str()),
        run.id.as_str()
    );
}

/// The payload of a `verify` whose callback panics with the text that `text`
/// makes. The panic hook prints nothing while the callback runs, so that the
/// test output holds no text of about 1 MB.
fn panic_payload_of(rule: &str, text: impl FnOnce() -> String) -> String {
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = verify(rule, "expiry-holds", move || -> Result<(), String> {
            std::panic::panic_any(text())
        });
    }));
    std::panic::set_hook(hook);
    *unwind.unwrap_err().downcast::<String>().unwrap()
}

#[test]
#[verifies("rule_rust_verification_cuts_a_long_failure_text", examples)]
fn a_failure_text_over_the_run_budget_is_stored_cut_and_returned_whole() {
    let (dir, _guard) = repository();
    let rule = seeded_rule(dir.path());
    let fits = verify(&rule, "expiry-holds", || {
        Err::<(), String>(text_for_row(dir.path(), BUDGET))
    })
    .unwrap_err()
    .to_string();
    assert_same_text(newest_run(dir.path()).error.as_deref(), &fits);

    let error = verify(&rule, "expiry-holds", || {
        Err::<(), String>(text_for_row(dir.path(), BUDGET + 1))
    })
    .unwrap_err();

    let run = newest_run(dir.path());
    let text = "x".repeat(fits.len() + 1);
    assert_same_text(Some(&error.to_string()), &text);
    assert_stored_cut(&run, &text, BUDGET);
    assert_evidence_gives(dir.path(), &rule, &run);
}

#[test]
#[verifies("rule_rust_verification_cuts_a_long_failure_text", examples)]
fn a_panic_text_over_the_run_budget_is_stored_cut_and_the_panic_goes_on_whole() {
    let (dir, _guard) = repository();
    let rule = seeded_rule(dir.path());
    let fits = panic_payload_of(&rule, || text_for_row(dir.path(), BUDGET));
    assert_same_text(newest_run(dir.path()).error.as_deref(), &fits);

    let payload = panic_payload_of(&rule, || text_for_row(dir.path(), BUDGET + 1));

    let run = newest_run(dir.path());
    let text = "x".repeat(fits.len() + 1);
    assert_same_text(Some(&payload), &text);
    assert_stored_cut(&run, &text, BUDGET);
    assert_evidence_gives(dir.path(), &rule, &run);
}

/// The stored line keeps a member that the run type does not know, and the
/// cut leaves room for it.
#[test]
#[verifies("rule_rust_verification_cuts_a_long_failure_text", examples)]
fn a_cut_text_leaves_room_for_a_member_of_the_stored_line_that_the_run_type_does_not_know() {
    let (dir, _guard) = repository();
    let rule = seeded_rule(dir.path());
    let path = runs_file(dir.path());
    let note = format!(",\"note\":\"{}\"}}", "n".repeat(5_000));
    let mut text = String::new();

    let error = verify(&rule, "expiry-holds", || {
        let line = std::fs::read_to_string(&path).unwrap();
        let running = line.trim_end().strip_suffix('}').unwrap().to_owned() + &note;
        std::fs::write(&path, running + "\n").unwrap();
        text = text_for_row(dir.path(), BUDGET + 1);
        Err::<(), String>(text.clone())
    })
    .unwrap_err();

    assert_same_text(Some(&error.to_string()), &text);
    let stored = std::fs::read_to_string(&path).unwrap();
    let line = stored.strip_suffix('\n').unwrap();
    assert_eq!(line.len(), BUDGET);
    assert!(line.ends_with(&note));
    assert_stored_cut(&newest_run(dir.path()), &text, BUDGET - (note.len() - 1));
}

/// For each character that the stored JSON string escapes or that is outside
/// the basic plane, the cut falls just before that character, with room left
/// that is one byte shorter than the stored form of the character.
#[test]
#[verifies("rule_rust_verification_cuts_a_long_failure_text", examples)]
fn a_cut_before_an_escaped_or_a_four_byte_character_keeps_a_whole_character() {
    for (character, stored) in [('"', 2), ('\\', 2), ('\u{1}', 6), ('😀', 4)] {
        let (dir, _guard) = repository();
        let rule = seeded_rule(dir.path());
        let mut text = String::new();

        let error = verify(&rule, "expiry-holds", || {
            let free = BUDGET - row_without_text(dir.path());
            let count = free / stored + 2;
            let full = count * character.len_utf8();
            let mark = format!("… [cut: {full} of {full} bytes]").len();
            let room = free - mark;
            text = "x".repeat((room - (stored - 1)) % stored);
            text.extend(std::iter::repeat_n(character, count));
            Err::<(), String>(text.clone())
        })
        .unwrap_err();

        assert_same_text(Some(&error.to_string()), &text);
        let run = newest_run(dir.path());
        assert_stored_cut(&run, &text, BUDGET - (stored - 1));
        let kept = run.error.as_deref().unwrap().rfind("… [cut: ").unwrap();
        assert!(text[kept..].starts_with(character), "{character:?}");
    }
}
