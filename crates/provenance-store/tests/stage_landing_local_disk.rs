//! What a stage writes on the local disk adapter. Unix only: the tests take
//! the write permission from the folder of whole-set transactions, so a
//! stage that makes a transaction fails.
#![cfg(all(unix, feature = "host"))]

#[path = "support/initialization.rs"]
mod initialization;

use camino::Utf8Path;
use provenance_macros::verifies;
use provenance_store::layout::ProvenanceLayout;
use provenance_store::repository::Repository;
use provenance_store::write_error::{WriteError, WriteFailure};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

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

/// An initialized repository whose stages cannot make a transaction.
fn repository() -> (tempfile::TempDir, ProvenanceLayout, Repository, ReadOnly) {
    let directory = tempfile::tempdir().unwrap();
    let layout = ProvenanceLayout::new(Utf8Path::from_path(directory.path()).unwrap());
    initialization::initialize(&layout, &[]);
    let repository = Repository::open(directory.path(), "default").unwrap();
    let read_only = ReadOnly::new(layout.import_transactions_dir().into_std_path_buf());
    (directory, layout, repository, read_only)
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
