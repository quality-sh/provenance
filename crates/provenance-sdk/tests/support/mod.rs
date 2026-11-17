//! A repository for the tests of `verify`: `verify` finds it through
//! `PROVENANCE_REPO`, and its working tree holds the test file that a run
//! cites.

use std::sync::{Mutex, MutexGuard, PoisonError};

use provenance_core::{Manifest, RepoPathPrefix, ScopeId, VerificationRun};
use provenance_sdk::{operations, Repository};
use provenance_store::layout::ProvenanceLayout;

/// Serializes tests that set process environment variables.
static ENV_LOCK: Mutex<()> = Mutex::new(());

/// A repository whose working tree holds `contents` as the test file `file`
/// of the SDK, and that `verify` finds while the guard lives.
pub fn repository(file: &str, contents: &str) -> (tempfile::TempDir, MutexGuard<'static, ()>) {
    // A failed test leaves the lock poisoned, and the next test still runs.
    let guard = ENV_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
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
    let source = root.join("crates/provenance-sdk/tests").join(file);
    std::fs::create_dir_all(source.parent().unwrap()).unwrap();
    std::fs::write(source, contents).unwrap();
    std::env::set_var("PROVENANCE_REPO", root.as_str());
    (dir, guard)
}

/// The id of the Rule `expiry`, which a typed spec in the repository at
/// `root` declares.
pub fn seeded_rule(root: &std::path::Path) -> String {
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
    let result = operations::apply(&open(root), input).unwrap();
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

/// The repository at `root`.
pub fn open(root: &std::path::Path) -> Repository {
    let root = camino::Utf8PathBuf::from_path_buf(root.to_path_buf()).unwrap();
    Repository::open(&root, "default").unwrap()
}

/// The stored runs of the repository at `root`.
pub fn recorded_runs(root: &std::path::Path) -> Vec<VerificationRun> {
    operations::verification_runs(&open(root), None).unwrap()
}
