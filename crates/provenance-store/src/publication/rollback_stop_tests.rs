//! A source edit whose state install or file install fails rolls its state
//! back. These tests end that roll back at each point between its first move
//! and the clear of its marker, and then enter the section again.
//!
//! The probe before the roll back takes the write permission from one
//! directory, so that one step of the roll back fails with no change on
//! disk. The disk is then as a stop at that step leaves it.

use super::backup_tests::{backups, marked_backup};
use super::source_edit_recovery_tests::{
    canonical_root, fixture, prepared_files, published, try_publish, AFTER, BEFORE,
};
use crate::stopped_child::names;
use crate::{
    layout::ProvenanceLayout,
    publication::{with_repository_publication, SourceEditRecoveryFailure},
    test_probes,
    write_error::PublicationStarted,
};
use camino::{Utf8Path, Utf8PathBuf};
use std::os::unix::fs::PermissionsExt;

/// Runs a source edit whose step at the probe `failure` fails one time.
/// Before its roll back, the write permission goes from the directory that
/// `locked` gives. The write gives the error of the roll back, which tells
/// that the publication started.
fn roll_back_with_locked(
    root: &Utf8Path,
    failure: &'static str,
    fail: impl FnMut() -> anyhow::Result<()> + 'static,
    locked: fn(&ProvenanceLayout) -> Utf8PathBuf,
) {
    let directory = std::rc::Rc::new(std::cell::RefCell::new(None));
    let set = directory.clone();
    let lock_layout = ProvenanceLayout::new(root);
    test_probes::arm(failure, fail);
    test_probes::arm("state_before_rollback", move || {
        let path = locked(&lock_layout);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o500))?;
        *set.borrow_mut() = Some(path);
        Ok(())
    });

    let result = try_publish(root);
    test_probes::disarm(failure);
    test_probes::disarm("state_before_rollback");
    let path = directory
        .borrow_mut()
        .take()
        .expect("the roll back started");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();

    let error = result.unwrap_err();
    assert!(
        error.downcast_ref::<PublicationStarted>().is_some(),
        "{error:#}"
    );
}

fn fail_file_install() -> anyhow::Result<()> {
    anyhow::bail!("injected install failure")
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

/// The state value at the live path, in the backup and in the stage of
/// `transaction`, each when present.
type States = [Option<Vec<u8>>; 3];

fn states(layout: &ProvenanceLayout, transaction: &Utf8Path) -> States {
    let value = |state: Utf8PathBuf| std::fs::read(state.join("value")).ok();
    [
        value(layout.state_dir()),
        value(transaction.join("backup-state")),
        value(ProvenanceLayout::new(transaction.join("staged-repo")).state_dir()),
    ]
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
    assert_eq!(backups(root), Vec::<String>::new());
}

/// A stop after the roll back moves the new state from the live path and
/// before it moves the backup to the live path.
#[test]
fn a_stop_between_the_two_moves_of_a_roll_back_leaves_one_whole_state() {
    let temp = fixture();
    let root_path = canonical_root(&temp);
    let root = root_path.as_path();
    let layout = ProvenanceLayout::new(root);
    roll_back_with_locked(
        root,
        "repository_file_after_backup_check",
        fail_file_install,
        transaction,
    );
    let stage = transaction(&layout);
    assert_eq!(marked_backup(&layout).1, "state_installed");
    assert_eq!(std::fs::read(root.join("source.txt")).unwrap(), BEFORE);
    let setup = [None, Some(BEFORE.to_vec()), Some(AFTER.to_vec())];
    assert_eq!(states(&layout, &stage), setup);

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
    roll_back_with_locked(
        root,
        "repository_file_after_backup_check",
        fail_file_install,
        cache,
    );
    let stage = transaction(&layout);
    assert_eq!(marked_backup(&layout).1, "state_installed");
    assert_eq!(std::fs::read(root.join("source.txt")).unwrap(), BEFORE);
    let setup = [Some(BEFORE.to_vec()), None, Some(AFTER.to_vec())];
    assert_eq!(states(&layout, &stage), setup);

    with_repository_publication(&layout, |_| Ok(())).unwrap();

    assert_completed(root);
}

/// The state install fails after the backup move. A stop in the roll back
/// before the clear of its marker leaves the backup at the live path and
/// the staged state in the transaction under the `BackupCreated` marker.
#[test]
fn a_stop_before_the_clear_of_a_roll_back_of_the_state_install_leaves_one_whole_state() {
    let temp = fixture();
    let root_path = canonical_root(&temp);
    let root = root_path.as_path();
    let layout = ProvenanceLayout::new(root);
    let fail = || -> anyhow::Result<()> { anyhow::bail!("injected state install failure") };
    roll_back_with_locked(root, "state_before_install", fail, cache);
    let stage = transaction(&layout);
    assert_eq!(marked_backup(&layout).1, "backup_created");
    assert_eq!(std::fs::read(root.join("source.txt")).unwrap(), BEFORE);
    let setup = [Some(BEFORE.to_vec()), None, Some(AFTER.to_vec())];
    assert_eq!(states(&layout, &stage), setup);

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
    roll_back_with_locked(
        root,
        "repository_file_after_backup_check",
        fail_file_install,
        cache,
    );
    let stage = transaction(&layout);
    test_probes::crash_at("state_after_backup_rename");

    let stopped = with_repository_publication(&layout, |_| Ok(()));
    test_probes::disarm("state_after_backup_rename");
    let stopped_states = states(&layout, &stage);
    let stopped_phase = layout
        .source_edit_marker_path()
        .exists()
        .then(|| marked_backup(&layout).1);
    with_repository_publication(&layout, |_| Ok(())).unwrap();

    assert_completed(root);
    let setup = [None, Some(BEFORE.to_vec()), Some(AFTER.to_vec())];
    assert_eq!(stopped_states, setup);
    assert_eq!(stopped_phase.as_deref(), Some("state_installed"));
    assert_eq!(
        format!("{:#}", stopped.unwrap_err()),
        "injected crash at state_after_backup_rename"
    );
}

/// A person changes the target while the file install runs, so the install
/// fails and the roll back starts. A stop between its two moves leaves no
/// live state. The section entry puts the state after the edit at the live
/// path, and then refuses for the changed file.
#[test]
fn a_person_change_and_a_stop_in_the_roll_back_leave_a_live_state_and_the_refusal() {
    let temp = fixture();
    let root_path = canonical_root(&temp);
    let root = root_path.as_path();
    let layout = ProvenanceLayout::new(root);
    let person_layout = ProvenanceLayout::new(root);
    let fail = move || {
        let (backup, _) = marked_backup(&person_layout);
        std::fs::write(backup, b"person\n")?;
        Ok(())
    };
    roll_back_with_locked(root, "repository_file_after_displace", fail, transaction);
    let stage = transaction(&layout);
    let setup = [None, Some(BEFORE.to_vec()), Some(AFTER.to_vec())];
    assert_eq!(states(&layout, &stage), setup);
    assert_eq!(std::fs::read(root.join("source.txt")).unwrap(), b"person\n");

    let error = with_repository_publication(&layout, |_| Ok(())).unwrap_err();

    let completed = [Some(AFTER.to_vec()), Some(BEFORE.to_vec()), None];
    assert_eq!(states(&layout, &stage), completed);
    assert_eq!(marked_backup(&layout).1, "state_installed");
    assert_eq!(std::fs::read(root.join("source.txt")).unwrap(), b"person\n");
    assert!(matches!(
        error.downcast_ref::<SourceEditRecoveryFailure>(),
        Some(SourceEditRecoveryFailure::ExternalChange { .. })
    ));
    assert_eq!(
        format!("{error:#}"),
        format!(
            "end the write that {} names: source file {} changed outside the pending \
             source-edit transaction",
            layout.source_edit_marker_path(),
            root.join("source.txt")
        )
    );
}
