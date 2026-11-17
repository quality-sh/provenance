//! A source edit whose file install fails rolls its state back. These tests
//! end that roll back at each point between its first move and the clear of
//! its marker, and then enter the section again.
//!
//! The probe before the roll back takes the write permission from one
//! directory, so that one step of the roll back fails with no change on
//! disk. The disk is then as a stop at that step leaves it.

use super::source_edit_recovery_tests::{
    canonical_root, fixture, prepared_files, published, try_publish, AFTER, BEFORE,
};
use crate::stopped_child::names;
use crate::{layout::ProvenanceLayout, publication::with_repository_publication, test_probes};
use camino::{Utf8Path, Utf8PathBuf};
use std::os::unix::fs::PermissionsExt;

/// Runs a source edit whose file install fails one time. Before its roll
/// back, the write permission goes from the directory that `locked` gives.
fn roll_back_with_locked(root: &Utf8Path, locked: fn(&ProvenanceLayout) -> Utf8PathBuf) {
    let layout = ProvenanceLayout::new(root);
    let directory = std::rc::Rc::new(std::cell::RefCell::new(None));
    let set = directory.clone();
    let lock_layout = ProvenanceLayout::new(root);
    test_probes::crash_at("repository_file_after_backup_check");
    test_probes::arm("state_before_rollback", move || {
        let path = locked(&lock_layout);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o500))?;
        *set.borrow_mut() = Some(path);
        Ok(())
    });

    let result = try_publish(root);
    test_probes::disarm("repository_file_after_backup_check");
    test_probes::disarm("state_before_rollback");
    let path = directory
        .borrow_mut()
        .take()
        .expect("the roll back started");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();

    assert!(result.is_err());
    assert_eq!(phase(&layout), "state_installed");
    assert_eq!(std::fs::read(root.join("source.txt")).unwrap(), BEFORE);
}

/// The one source-edit transaction: its second move of the roll back fails.
fn transaction(layout: &ProvenanceLayout) -> Utf8PathBuf {
    let transactions = layout.source_edit_transactions_dir();
    let names = names(&transactions);
    assert_eq!(names.len(), 1);
    transactions.join(&names[0])
}

/// The cache: the clear of the marker fails.
fn cache(layout: &ProvenanceLayout) -> Utf8PathBuf {
    layout.cache_dir()
}

fn phase(layout: &ProvenanceLayout) -> String {
    let marker: serde_json::Value =
        serde_json::from_slice(&std::fs::read(layout.source_edit_marker_path()).unwrap()).unwrap();
    marker["phase"].as_str().unwrap().to_owned()
}

/// The state after the edit, and no data of the write.
fn assert_completed(root: &Utf8Path) {
    let layout = ProvenanceLayout::new(root);
    assert_eq!(published(root), (AFTER.to_vec(), AFTER.to_vec()));
    assert!(!layout.source_edit_marker_path().exists());
    assert_eq!(
        names(&layout.source_edit_transactions_dir()),
        Vec::<String>::new()
    );
    assert_eq!(prepared_files(root), Vec::<String>::new());
}

/// A stop after the roll back moves the new state from the live path and
/// before it moves the backup to the live path.
#[test]
fn a_stop_between_the_two_moves_of_a_roll_back_leaves_one_whole_state() {
    let temp = fixture();
    let root_path = canonical_root(&temp);
    let root = root_path.as_path();
    let layout = ProvenanceLayout::new(root);
    roll_back_with_locked(root, transaction);
    assert!(!layout.state_dir().exists(), "the live state is moved");

    with_repository_publication(&layout, |_| Ok(())).unwrap();

    assert_completed(root);
}

/// A stop after the roll back puts the backup at the live path and before
/// the clear of the marker.
#[test]
fn a_stop_before_the_clear_of_the_marker_of_a_roll_back_leaves_one_whole_state() {
    let temp = fixture();
    let root_path = canonical_root(&temp);
    let root = root_path.as_path();
    let layout = ProvenanceLayout::new(root);
    roll_back_with_locked(root, cache);
    assert_eq!(
        std::fs::read(layout.state_dir().join("value")).unwrap(),
        BEFORE
    );

    with_repository_publication(&layout, |_| Ok(())).unwrap();

    assert_completed(root);
}

/// After a roll back to the backup, the section entry stops after it moves
/// the live state to the backup. The next section entry completes the edit.
#[test]
fn a_stop_in_the_recovery_of_a_rolled_back_edit_leaves_one_whole_state() {
    let temp = fixture();
    let root_path = canonical_root(&temp);
    let root = root_path.as_path();
    let layout = ProvenanceLayout::new(root);
    roll_back_with_locked(root, cache);
    test_probes::crash_at("state_after_backup_rename");

    let stopped = with_repository_publication(&layout, |_| Ok(()));
    test_probes::disarm("state_after_backup_rename");

    assert!(stopped.is_err());
    assert_eq!(phase(&layout), "state_installed");
    with_repository_publication(&layout, |_| Ok(())).unwrap();
    assert_completed(root);
}
