//! The failing-verification corpus for the Rust side (V11): an error and
//! a panic both record `failed`, then propagate.

use std::sync::{Mutex, MutexGuard};

use provenance_core::{Manifest, RepoPathPrefix, ScopeId, VerificationRunStatus};
use provenance_macros::verifies;
use provenance_sdk::{operations, verify, Repository};
use provenance_store::layout::ProvenanceLayout;

/// Serializes tests that set process environment variables.
static ENV_LOCK: Mutex<()> = Mutex::new(());

fn repository() -> (tempfile::TempDir, MutexGuard<'static, ()>) {
    let guard = ENV_LOCK.lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let root =
        camino::Utf8PathBuf::from_path_buf(dunce::canonicalize(dir.path()).unwrap()).unwrap();
    let layout = ProvenanceLayout::new(root.clone());
    std::fs::create_dir_all(layout.manifest_path().parent().unwrap()).unwrap();
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_string(&Manifest::default_with_scope(
            ScopeId::new("default").unwrap(),
            RepoPathPrefix::new("."),
        ))
        .unwrap(),
    )
    .unwrap();
    let source = root.join("crates/provenance-sdk/tests/verification.rs");
    std::fs::create_dir_all(source.parent().unwrap()).unwrap();
    std::fs::write(source, include_str!("verification.rs")).unwrap();
    std::env::set_var("PROVENANCE_REPO", root.as_str());
    (dir, guard)
}

fn seeded_rule(root: &std::path::Path) -> String {
    let input: provenance_sdk::TypedSpecInput = serde_json::from_value(serde_json::json!({
        "spec": "share-links",
        "declared_by": "spec://rust",
        "requirements": [{
            "key": "sharing",
            "statement": "Users can securely share documentation"
        }],
        "rules": [{
            "key": "expiry",
            "requirement": "sharing",
            "statement": "Share links expire within 30 days"
        }]
    }))
    .unwrap();
    let root = camino::Utf8PathBuf::from_path_buf(root.to_path_buf()).unwrap();
    let result = operations::apply(&Repository::open(&root, "default").unwrap(), input).unwrap();
    result
        .resources
        .iter()
        .find(|resource| {
            matches!(
                resource.kind,
                provenance_store::state_store::TypedResourceKind::Rule
            )
        })
        .unwrap()
        .id
        .as_str()
        .to_string()
}

fn recorded_runs(root: &std::path::Path) -> Vec<provenance_core::VerificationRun> {
    let root = camino::Utf8PathBuf::from_path_buf(root.to_path_buf()).unwrap();
    operations::verification_runs(&Repository::open(&root, "default").unwrap(), None).unwrap()
}

#[test]
fn a_passing_callback_records_passed() {
    let (dir, _guard) = repository();
    let rule = seeded_rule(dir.path());

    verify(&rule, "expiry-holds", || Ok::<(), String>(())).unwrap();

    let runs = recorded_runs(dir.path());
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].status, VerificationRunStatus::Passed);
    assert_eq!(runs[0].declared_by, "ci://rust");
    assert_eq!(
        runs[0].file.as_deref().map(camino::Utf8Path::as_str),
        Some("crates/provenance-sdk/tests/verification.rs"),
        "the call site file is the binding file"
    );
}

#[test]
#[verifies("rule_rust_verification_records_failure_first", examples)]
fn a_failing_callback_records_failed_then_propagates() {
    let (dir, _guard) = repository();
    let rule = seeded_rule(dir.path());

    let error = verify(&rule, "expiry-holds", || {
        Err::<(), String>("thirty-one days".to_string())
    })
    .unwrap_err();

    assert_eq!(error.to_string(), "thirty-one days");
    let runs = recorded_runs(dir.path());
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].status, VerificationRunStatus::Failed);
    assert_eq!(runs[0].error.as_deref(), Some("thirty-one days"));
}

#[test]
#[verifies("rule_rust_verification_records_failure_first", examples)]
fn an_unwinding_callback_records_failed_then_propagates() {
    let (dir, _guard) = repository();
    let rule = seeded_rule(dir.path());

    let unwind = std::panic::catch_unwind(|| {
        let _ = verify(&rule, "expiry-holds", || -> Result<(), String> {
            panic!("expiry assertion failed")
        });
    })
    .unwrap_err();

    assert_eq!(
        unwind.downcast_ref::<&str>(),
        Some(&"expiry assertion failed")
    );
    let runs = recorded_runs(dir.path());
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].status, VerificationRunStatus::Failed);
    assert_eq!(runs[0].error.as_deref(), Some("expiry assertion failed"));
}

/// The bytes of one run row that the reads of the runs give. The store owns
/// the number in `read_budget::resource_record_bytes`.
const BUDGET: usize = 1_048_576 - 16_384;

/// A failure text that makes the failed row of the running run `row` bytes
/// long. The callback reads the run that `verify` started.
fn text_for_row(root: &std::path::Path, row: usize) -> String {
    let mut run = recorded_runs(root)
        .into_iter()
        .find(|run| run.status == VerificationRunStatus::Running)
        .unwrap();
    run.status = VerificationRunStatus::Failed;
    run.completed_at = Some(run.started_at);
    run.error = Some(String::new());
    "x".repeat(row - serde_json::to_vec(&run).unwrap().len())
}

/// The newest stored run, and the newest run that the evidence read of
/// `rule` gives.
fn newest_runs(
    root: &std::path::Path,
    rule: &str,
) -> (
    provenance_core::VerificationRun,
    Option<provenance_core::VerificationRun>,
) {
    let mut runs = recorded_runs(root);
    runs.sort_by(|left, right| {
        (left.started_at, left.id.as_str()).cmp(&(right.started_at, right.id.as_str()))
    });
    let utf8 = camino::Utf8PathBuf::from_path_buf(root.to_path_buf()).unwrap();
    let repository = Repository::open(&utf8, "default").unwrap();
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
            &repository,
            operations::read_policy::ReadPolicy::default(),
            query,
        ))
        .unwrap();
    (
        runs.pop().unwrap(),
        evidence.result.latest_verification_run,
    )
}

/// Asserts that `run` failed inside the budget, and that its stored text is
/// the start of `text` with a mark that gives the byte count of the stored
/// part and of `text`.
fn assert_stored_cut(run: &provenance_core::VerificationRun, text: &str) {
    assert_eq!(run.status, VerificationRunStatus::Failed);
    assert!(serde_json::to_vec(run).unwrap().len() <= BUDGET);
    let stored = run.error.as_deref().unwrap();
    let (kept, mark) = stored.split_at(stored.rfind("… [cut: ").unwrap());
    assert!(text.starts_with(kept));
    assert!(kept.len() < text.len());
    assert_eq!(
        mark,
        format!("… [cut: {} of {} bytes]", kept.len(), text.len())
    );
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
    let (run, _) = newest_runs(dir.path(), &rule);
    assert_eq!(run.error.as_deref(), Some(fits.as_str()));

    let error = verify(&rule, "expiry-holds", || {
        Err::<(), String>(text_for_row(dir.path(), BUDGET + 1))
    })
    .unwrap_err();

    let (run, evidence) = newest_runs(dir.path(), &rule);
    let text = "x".repeat(fits.len() + 1);
    assert_eq!(error.to_string(), text);
    assert_stored_cut(&run, &text);
    assert_eq!(evidence, Some(run));
}

#[test]
#[verifies("rule_rust_verification_cuts_a_long_failure_text", examples)]
fn a_panic_text_over_the_run_budget_is_stored_cut_and_the_panic_goes_on_whole() {
    let (dir, _guard) = repository();
    let rule = seeded_rule(dir.path());
    let fits = std::panic::catch_unwind(|| {
        let _ = verify(&rule, "expiry-holds", || -> Result<(), String> {
            std::panic::panic_any(text_for_row(dir.path(), BUDGET))
        });
    })
    .unwrap_err();
    let fits = fits.downcast_ref::<String>().unwrap();
    let (run, _) = newest_runs(dir.path(), &rule);
    assert_eq!(run.error.as_ref(), Some(fits));

    let unwind = std::panic::catch_unwind(|| {
        let _ = verify(&rule, "expiry-holds", || -> Result<(), String> {
            std::panic::panic_any(text_for_row(dir.path(), BUDGET + 1))
        });
    })
    .unwrap_err();

    let (run, evidence) = newest_runs(dir.path(), &rule);
    let text = "x".repeat(fits.len() + 1);
    assert_eq!(unwind.downcast_ref::<String>(), Some(&text));
    assert_stored_cut(&run, &text);
    assert_eq!(evidence, Some(run));
}
