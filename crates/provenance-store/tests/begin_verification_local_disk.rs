//! The start of a verification run on the local disk adapter, when the
//! process cannot write one directory of the cache. Unix only: the tests
//! take the write permission from a directory with its mode.
#![cfg(unix)]

use crate::discussion_support::base::initialization;
use crate::memory;

use camino::Utf8Path;
use memory::{begin_input, scope};
use provenance_core::{RequirementReview, VerificationRun};
use provenance_store::layout::ProvenanceLayout;
use provenance_store::operations::begin_verification;
use provenance_store::repository::Repository;
use provenance_store::write_error::WriteError;
use serde_json::{json, Value};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// Takes the write permission from a directory, and gives it back on drop.
struct ReadOnly(PathBuf);

impl ReadOnly {
    fn new(path: PathBuf) -> Self {
        std::fs::create_dir_all(&path).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o555)).unwrap();
        let guard = Self(path);
        assert!(
            std::fs::write(guard.0.join("probe"), b"x").is_err(),
            "the test user can write a read-only directory"
        );
        guard
    }
}

impl Drop for ReadOnly {
    fn drop(&mut self) {
        let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o755));
    }
}

/// An initialized repository on the local disk with the active Rule
/// `rule_expiry` and an open review of it: the statement of its Requirement
/// changed after the Rule was made.
fn repository(root: &Path) -> Repository {
    std::fs::write(root.join("evidence.rs"), "fn expiry() {}\n").unwrap();
    initialization::initialize(
        &ProvenanceLayout::new(Utf8Path::from_path(root).unwrap()),
        &[],
    );
    let repository = Repository::open(root, "default").unwrap();
    let store = repository.store();
    let requirement = json!({
        "actor": "ben", "origin": null,
        "create": {
            "scope_id": "default", "id": "req_expiry", "statement": "Links expire",
            "status": "discovery", "depends_on": [], "supersedes": []
        }
    });
    store
        .create_review_requirement(serde_json::from_value(requirement).unwrap())
        .unwrap();
    let rule = json!({
        "scope_id": "default", "id": "rule_expiry", "requirement_ids": ["req_expiry"],
        "resolution_ids": [], "statement": "Links expire.", "status": "active",
        "severity": "high"
    });
    store
        .create_rule(serde_json::from_value(rule).unwrap())
        .unwrap();
    for mut update in [json!({}), json!({"statement": "Share links expire"})] {
        update["scope_id"] = json!("default");
        update["id"] = json!("req_expiry");
        let etag = store
            .requirement_edit_state(&scope(), &memory::id("req_expiry"))
            .unwrap()
            .etag;
        let save = json!({
            "actor": "ben", "expected_etag": etag, "update": update, "relationships": null
        });
        store
            .save_requirement(serde_json::from_value(save).unwrap())
            .unwrap();
    }
    assert_eq!(store.open_requirement_reviews(&scope()).unwrap().len(), 1);
    repository
}

/// The answer body of a failed start and its status.
fn failure(repository: &Repository) -> (Value, u16) {
    let error = WriteError(begin_verification(repository, begin_input()).unwrap_err());
    (serde_json::to_value(&error).unwrap(), error.status())
}

fn reviews(repository: &Repository) -> Vec<RequirementReview> {
    repository
        .store()
        .list_requirement_reviews(&scope())
        .unwrap()
}

fn runs(repository: &Repository) -> Vec<VerificationRun> {
    repository.store().list_verification_runs(&scope()).unwrap()
}

fn bindings(repository: &Repository) -> usize {
    let store = repository.store();
    store.list_verification_bindings(&scope()).unwrap().len()
}

fn assert_permission_denied(answer: &Value, status: u16) {
    assert_eq!((&answer["kind"], status), (&json!("write_failed"), 500));
    let cause = answer["cause"].as_str().unwrap();
    assert!(
        cause.starts_with("Permission denied (os error 13)"),
        "{answer}"
    );
}

/// The local adapter lands a whole set on a copy of the state directory in
/// the cache. No Rule states the answer when it cannot make that copy, so
/// the test has no Rule binding.
#[test]
fn a_refused_set_on_local_disk_leaves_no_binding_no_run_row_and_an_open_review() {
    let root = tempfile::tempdir().unwrap();
    let repository = repository(root.path());
    let open = reviews(&repository);
    let _copies = ReadOnly::new(root.path().join(".provenance/cache/import-transactions"));

    let (answer, status) = failure(&repository);

    assert_permission_denied(&answer, status);
    assert_eq!(
        (
            bindings(&repository),
            runs(&repository),
            reviews(&repository)
        ),
        (0, Vec::new(), open)
    );
}

/// The run row goes into the cache after the binding and the cleared
/// reviews landed. No Rule states the state after a failed write of the
/// run row, so the test has no Rule binding.
#[test]
fn a_run_row_that_cannot_be_written_leaves_the_binding_and_the_cleared_review() {
    let root = tempfile::tempdir().unwrap();
    let repository = repository(root.path());
    let _cache = ReadOnly::new(root.path().join(".provenance/cache/scopes/default"));

    let (answer, status) = failure(&repository);

    assert_permission_denied(&answer, status);
    let cause = answer["cause"].as_str().unwrap();
    assert!(cause.contains(".tmp"), "{answer}");
    let cleared = reviews(&repository)[0].cleared_by_run.is_some();
    assert_eq!(
        (bindings(&repository), runs(&repository), cleared),
        (1, Vec::new(), true)
    );
}

/// A start after a failed run row adds its own row and keeps the binding.
/// The review stays cleared by the first start, which no row names. No Rule
/// states this repair, so the test has no Rule binding.
#[test]
fn a_start_after_a_failed_run_row_adds_its_row() {
    let root = tempfile::tempdir().unwrap();
    let repository = repository(root.path());
    let cache = ReadOnly::new(root.path().join(".provenance/cache/scopes/default"));
    failure(&repository);
    let first = reviews(&repository)[0].cleared_by_run.clone().unwrap();
    drop(cache);

    let run = begin_verification(&repository, begin_input()).unwrap();

    let cleared_by = reviews(&repository)[0].cleared_by_run.clone();
    assert_eq!(
        (bindings(&repository), runs(&repository), cleared_by),
        (1, vec![run], Some(first))
    );
}
