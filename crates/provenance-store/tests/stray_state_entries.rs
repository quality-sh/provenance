//! An entry under `.provenance/state` that is not the manifest or a shard is
//! not canonical state.
//!
//! A write through the storage port still lands and reports success, and
//! the view leaves the entry out. A shard that the process cannot read is
//! still an error.
#![cfg(unix)]

use camino::{Utf8Path, Utf8PathBuf};
use provenance_core::{
    RequirementStatus, RuleSeverity, RuleStatus, ScopeId, StableId, VerificationMethod,
};
use provenance_macros::verifies;
use provenance_store::layout::ProvenanceLayout;
use provenance_store::repository::Repository;
use provenance_store::state_store::{
    CreateRequirementInput, CreateRuleInput, MaterializeVerificationBindingInput,
};
use std::os::unix::fs::PermissionsExt;

#[path = "support/initialization.rs"]
mod initialization;

fn scope() -> ScopeId {
    ScopeId::new("default").unwrap()
}

fn id(value: &str) -> StableId {
    StableId::new(value).unwrap()
}

/// A repository with one Requirement and one Rule, created through the live
/// store.
fn repository() -> (tempfile::TempDir, Repository) {
    let dir = tempfile::tempdir().unwrap();
    let layout = ProvenanceLayout::new(Utf8Path::from_path(dir.path()).unwrap());
    initialization::initialize(&layout, &[]);
    let repository = Repository::open(dir.path(), "default").unwrap();
    let store = repository.store();
    store
        .create_requirement(CreateRequirementInput {
            scope_id: scope(),
            id: id("req_expiry"),
            statement: "Share links expire".into(),
            description: None,
            status: RequirementStatus::Active,
            domain_id: None,
            refines: Vec::new(),
            depends_on: Vec::new(),
            supersedes: Vec::new(),
            spawned_by: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
    store
        .create_rule(CreateRuleInput {
            archived_in_commit: None,
            scope_id: scope(),
            id: id("rule_expiry"),
            name: None,
            description: None,
            requirement_ids: vec![id("req_expiry")],
            resolution_ids: Vec::new(),
            statement: "Share links expire".into(),
            status: RuleStatus::Active,
            severity: RuleSeverity::Medium,
            source_document: None,
            source_section: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
    (dir, repository)
}

fn rules_dir(repository: &Repository) -> Utf8PathBuf {
    repository.layout().scopes_dir().join("default/rules")
}

/// Makes the file at `path` unreadable. Returns `false` when the process
/// can still read it, for example as root.
fn make_unreadable(path: &Utf8Path) -> bool {
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o000)).unwrap();
    std::fs::read(path).is_err()
}

/// A write that lands through the storage port reports success, and the
/// view lists the Rule and the new binding.
fn assert_write_and_view_work(repository: &Repository) {
    repository
        .store()
        .materialize_verification_binding(MaterializeVerificationBindingInput {
            scope_id: scope(),
            rule_id: id("rule_expiry"),
            key: "share-link-expiry".into(),
            method: VerificationMethod::Examples,
            declared_by: "test://typescript".into(),
            file: "tests/share-links.test.ts".into(),
            symbol: None,
        })
        .unwrap();
    let view = repository.view().unwrap();
    assert_eq!(view.list_rules(&scope()).unwrap().len(), 1);
    assert_eq!(view.list_verification_bindings(&scope()).unwrap().len(), 1);
}

#[cfg(target_os = "linux")]
#[test]
#[verifies("rule_canonical_state_is_read_as_one_view", examples)]
fn a_stray_name_that_is_not_utf8_is_left_out() {
    use std::os::unix::ffi::OsStrExt;
    let (_dir, repository) = repository();
    let name = std::ffi::OsStr::from_bytes(b"stray-\xff.jsonl");
    std::fs::write(rules_dir(&repository).as_std_path().join(name), b"x").unwrap();
    std::fs::write(
        repository.layout().state_dir().as_std_path().join(name),
        b"x",
    )
    .unwrap();
    assert_write_and_view_work(&repository);
}

#[test]
#[verifies("rule_canonical_state_is_read_as_one_view", examples)]
fn a_stray_name_with_a_backslash_is_left_out() {
    let (_dir, repository) = repository();
    std::fs::write(rules_dir(&repository).join("stray\\name.jsonl"), b"x").unwrap();
    std::fs::write(repository.layout().state_dir().join("a\\b"), b"x").unwrap();
    assert_write_and_view_work(&repository);
}

#[test]
#[verifies("rule_canonical_state_is_read_as_one_view", examples)]
fn an_unreadable_stray_file_is_left_out() {
    let (_dir, repository) = repository();
    let stray = rules_dir(&repository).join("notes.txt");
    std::fs::write(&stray, b"x").unwrap();
    if !make_unreadable(&stray) {
        return;
    }
    assert_write_and_view_work(&repository);
}

#[test]
#[verifies("rule_canonical_state_is_read_as_one_view", examples)]
fn an_unreadable_shard_is_an_error() {
    let (_dir, repository) = repository();
    let shard = rules_dir(&repository).join("rule.jsonl");
    if !make_unreadable(&shard) {
        return;
    }
    assert!(repository.view().is_err());
}
