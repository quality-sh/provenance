//! The failing-verification corpus for the Rust side (V11): an error and
//! a panic both record `failed`, then propagate.

use std::sync::{Mutex, MutexGuard};

use provenance_core::SUPPORTED_SCHEMA_VERSION;
use provenance_core::{Manifest, RepoPathPrefix, ScopeId, VerificationRunStatus};
use provenance_macros::verifies;
use provenance_sdk::{operations, verify};
use provenance_store::layout::ProvenanceLayout;

/// Serializes tests that set process environment variables.
static ENV_LOCK: Mutex<()> = Mutex::new(());

fn repository() -> (tempfile::TempDir, MutexGuard<'static, ()>) {
    let guard = ENV_LOCK.lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let root = camino::Utf8PathBuf::from_path_buf(dir.path().canonicalize().unwrap()).unwrap();
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
    std::env::set_var("PROVENANCE_REPO", root.as_str());
    (dir, guard)
}

fn seeded_rule(root: &std::path::Path) -> String {
    let input: provenance_sdk::TypedSpecInput = serde_json::from_value(serde_json::json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
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
    let result = operations::apply(Some(root), &ScopeId::new("default").unwrap(), input).unwrap();
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
    operations::verification_runs(Some(root), &ScopeId::new("default").unwrap(), None).unwrap()
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
