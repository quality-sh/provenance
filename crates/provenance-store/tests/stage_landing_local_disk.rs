//! What a stage writes on the local disk adapter. Unix only: the tests take
//! the write permission from the folder of whole-set transactions, so a
//! stage that makes a transaction fails.
#![cfg(all(unix, feature = "host"))]

#[path = "support/initialization.rs"]
mod initialization;

use camino::Utf8Path;
use provenance_core::{RequirementStatus, ScopeId, StableId};
use provenance_macros::verifies;
use provenance_store::layout::ProvenanceLayout;
use provenance_store::repository::Repository;
use provenance_store::state_store::{CreateRequirementInput, UpdateRequirementInput};
use provenance_store::write_error::{WriteError, WriteFailure};
use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// Takes the write permission from a folder, and gives it back on drop.
struct ReadOnly(PathBuf);

impl ReadOnly {
    fn new(path: PathBuf) -> Self {
        std::fs::create_dir_all(&path).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o500)).unwrap();
        let guard = Self(path);
        assert!(
            std::fs::create_dir(guard.0.join("probe")).is_err(),
            "the test user can write a read-only folder"
        );
        guard
    }
}

impl Drop for ReadOnly {
    fn drop(&mut self) {
        let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o755));
    }
}

/// An initialized repository with the Requirement `req_owner`, whose stages
/// cannot make a transaction.
fn repository() -> (tempfile::TempDir, ProvenanceLayout, Repository, ReadOnly) {
    let directory = tempfile::tempdir().unwrap();
    let layout = ProvenanceLayout::new(Utf8Path::from_path(directory.path()).unwrap());
    initialization::initialize(&layout, &[]);
    let repository = Repository::open(directory.path(), "default").unwrap();
    repository
        .store()
        .create_requirement(requirement("req_owner"))
        .unwrap();
    let read_only = ReadOnly::new(layout.import_transactions_dir().into_std_path_buf());
    (directory, layout, repository, read_only)
}

fn requirement(id: &str) -> CreateRequirementInput {
    CreateRequirementInput {
        scope_id: ScopeId::new("default").unwrap(),
        id: StableId::new(id).unwrap(),
        statement: "Share links have an owner".into(),
        description: None,
        status: RequirementStatus::Active,
        refines: Vec::new(),
        depends_on: Vec::new(),
        supersedes: Vec::new(),
        spawned_by: None,
        origin_thread: None,
        origin_message: None,
    }
}

/// Every file below the state directory, with its bytes.
fn state_files(layout: &ProvenanceLayout) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(directory: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                visit(&path, files);
            } else {
                files.insert(path.clone(), std::fs::read(&path).unwrap());
            }
        }
    }
    let mut files = BTreeMap::new();
    visit(layout.state_dir().as_std_path(), &mut files);
    files
}

#[test]
#[verifies("rule_empty_write_set_writes_no_data", examples)]
fn a_stage_that_changes_no_key_writes_nothing() {
    let (_directory, layout, repository, _read_only) = repository();
    let before = std::fs::read(layout.manifest_path()).unwrap();

    repository.stage(false, None, |_| Ok(())).unwrap();

    assert!(!layout.publication_marker_path().exists());
    assert_eq!(std::fs::read(layout.manifest_path()).unwrap(), before);
}

/// A shard that changes on the disk while the stage runs moves its base.
#[test]
#[verifies("rule_adapter_refuses_a_set_on_a_moved_base", examples)]
#[verifies("rule_moved_base_refusal_is_state_changed", examples)]
fn a_stage_that_changes_no_key_on_a_moved_base_is_state_changed() {
    let (_directory, layout, repository, _read_only) = repository();
    let mut moved = std::fs::read(layout.manifest_path()).unwrap();
    moved.push(b'\n');

    let refused = repository
        .stage(false, None, |_| {
            std::fs::write(layout.manifest_path(), &moved)?;
            Ok(())
        })
        .unwrap_err();

    let refused = WriteError(refused);
    assert!(
        matches!(refused.safe(), WriteFailure::StateChanged { .. }),
        "{refused:#}"
    );
    assert_eq!(refused.status(), 409);
    assert!(!layout.publication_marker_path().exists());
    assert_eq!(std::fs::read(layout.manifest_path()).unwrap(), moved);
}

/// A stage that changes one shard lands it with no transaction.
#[test]
#[verifies("rule_store_sends_the_smallest_write_set", examples)]
fn a_stage_that_changes_one_key_lands_with_no_transaction() {
    let (_directory, layout, repository, _read_only) = repository();
    let update = UpdateRequirementInput {
        scope_id: ScopeId::new("default").unwrap(),
        id: StableId::new("req_owner").unwrap(),
        expected_etag: None,
        declared_by: None,
        statement: None,
        description: Some("Each share link names its owner.".into()),
        status: None,
        archived_in_commit: None,
        clear_fields: Vec::new(),
    };

    let updated = repository.store().update_requirement(update).unwrap();

    assert_eq!(
        updated.description.as_deref(),
        Some("Each share link names its owner.")
    );
    assert!(!layout.publication_marker_path().exists());
}

/// A stage that changes two shards needs a transaction, so here it fails
/// and changes no file.
#[test]
#[verifies("rule_stage_lands_as_one_whole_set", examples)]
fn a_stage_that_changes_two_keys_needs_a_transaction() {
    let (_directory, layout, repository, _read_only) = repository();
    let before = state_files(&layout);

    let refused = repository
        .store()
        .create_requirement(requirement("req_second"))
        .unwrap_err();

    let refused = WriteError(refused);
    assert!(
        matches!(refused.safe(), WriteFailure::WriteFailed { .. }),
        "{refused:#}"
    );
    assert_eq!(state_files(&layout), before);
}
