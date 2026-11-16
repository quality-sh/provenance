//! The failing-verification corpus for the Rust side (V11): an error and
//! a panic both record `failed`, then propagate.

mod support;

use provenance_core::VerificationRunStatus;
use provenance_macros::verifies;
use provenance_sdk::{operations, verify};
use provenance_store::state_store::CompleteVerificationInput;
use provenance_store::write_error::{WriteError, WriteFailure};
use support::{recorded_runs, seeded_rule};

fn repository() -> (tempfile::TempDir, std::sync::MutexGuard<'static, ()>) {
    support::repository("verification.rs", include_str!("verification.rs"))
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

/// Completes the running run of the repository at `root` as passed, as a
/// callback that completes its own run does. The failed completion of
/// `verify` that follows is then refused.
fn complete_the_running_run(root: &std::path::Path) -> String {
    let run = recorded_runs(root)
        .into_iter()
        .find(|run| run.status == VerificationRunStatus::Running)
        .unwrap();
    let input = CompleteVerificationInput {
        run: run.id.as_str().to_owned(),
        status: "passed".to_owned(),
        error: None,
    };
    operations::complete_verification(&support::open(root), input).unwrap();
    run.id.as_str().to_owned()
}

/// A panic payload of a type that only this file has.
#[derive(Debug, PartialEq)]
struct ExpiryPanic(&'static str);

/// Implementation aid with no Rule: no Rule states what `verify` gives when
/// the store refuses its failed completion.
#[test]
fn a_callback_error_after_a_refused_completion_holds_the_refusal_as_its_cause() {
    let (dir, _guard) = repository();
    let rule = seeded_rule(dir.path());
    let mut id = String::new();

    let error = verify(&rule, "expiry-holds", || {
        id = complete_the_running_run(dir.path());
        Err::<(), String>("thirty-one days".to_string())
    })
    .unwrap_err();

    assert_eq!(error.to_string(), "thirty-one days");
    assert_eq!(
        format!("{error:#}"),
        format!("thirty-one days: verification run `{id}` is already complete")
    );
    assert!(matches!(
        WriteError(error).safe(),
        WriteFailure::AlreadyComplete
    ));
    let runs = recorded_runs(dir.path());
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].status, VerificationRunStatus::Passed);
}

/// Implementation aid with no Rule: no Rule states what `verify` gives when
/// the store refuses its failed completion. The test
/// `a_refused_completion_after_a_panic_goes_to_the_standard_error` runs this
/// test in a child process and reads its standard error.
#[test]
fn a_panic_after_a_refused_completion_goes_on_with_its_own_payload() {
    let (dir, _guard) = repository();
    let rule = seeded_rule(dir.path());

    let unwind = std::panic::catch_unwind(|| {
        let _ = verify(&rule, "expiry-holds", || -> Result<(), String> {
            complete_the_running_run(dir.path());
            std::panic::panic_any(ExpiryPanic("expiry assertion failed"))
        });
    })
    .unwrap_err();

    assert_eq!(
        *unwind.downcast::<ExpiryPanic>().unwrap(),
        ExpiryPanic("expiry assertion failed")
    );
    let runs = recorded_runs(dir.path());
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].status, VerificationRunStatus::Passed);
}

/// Implementation aid with no Rule: no Rule states what `verify` gives when
/// the store refuses its failed completion.
#[test]
fn a_refused_completion_after_a_panic_goes_to_the_standard_error() {
    let child = "a_panic_after_a_refused_completion_goes_on_with_its_own_payload";
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", child, "--nocapture", "--test-threads=1"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    let refusals: Vec<&str> = stderr
        .lines()
        .filter(|line| line.contains("is not recorded as failed"))
        .collect();
    assert_eq!(refusals.len(), 1, "{stderr}");
    let id = refusals[0]
        .strip_prefix("verification run `")
        .and_then(|rest| rest.split('`').next())
        .unwrap();
    assert_eq!(
        refusals[0],
        format!(
            "verification run `{id}` is not recorded as failed: \
             verification run `{id}` is already complete"
        )
    );
}
