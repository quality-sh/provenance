//! The backup that a source edit keeps of its target while it installs the
//! replacement: a failure and a stop after the rename of the prepared file.

use super::source_edit_recovery_tests::{
    canonical_root, fixture, published, try_publish, AFTER, BEFORE, CHILD,
};
use crate::{
    layout::ProvenanceLayout,
    publication::with_repository_publication,
    stopped_child::{files_named, names, stop_in_child},
    test_probes,
};
use camino::{Utf8Path, Utf8PathBuf};
use provenance_macros::verifies;

/// The path of the backup that the marker of the source edit at `root` names,
/// and the phase of that marker.
fn marked_backup(layout: &ProvenanceLayout) -> (Utf8PathBuf, String) {
    let marker: serde_json::Value =
        serde_json::from_slice(&std::fs::read(layout.source_edit_marker_path()).unwrap()).unwrap();
    (
        layout.root().join(marker["backup_leaf"].as_str().unwrap()),
        marker["phase"].as_str().unwrap().to_owned(),
    )
}

fn backups(root: &Utf8Path) -> Vec<String> {
    files_named(root, ".source.txt.provenance-", ".backup")
}

/// After the rename of the prepared file onto the target, the write goes
/// only forward. The probe makes each removal of the backup fail until the
/// test disarms it: the write gives an error and keeps its marker, and the
/// next section entry does not open. The entry after that completes the
/// install.
#[test]
#[verifies("rule_failed_removal_of_stopped_write_names_its_path", examples)]
fn a_failed_removal_of_the_backup_keeps_the_marker_until_a_section_entry_removes_the_backup() {
    let temp = fixture();
    let root_path = canonical_root(&temp);
    let root = root_path.as_path();
    let layout = ProvenanceLayout::new(root);
    test_probes::arm("repository_file_unlink_backup", || {
        anyhow::bail!("injected unlink failure")
    });

    let written = try_publish(root);
    let mut ran = false;
    let entered = with_repository_publication(&layout, |_| {
        ran = true;
        Ok(())
    });
    test_probes::disarm("repository_file_unlink_backup");

    let (backup, phase) = marked_backup(&layout);
    let expected =
        format!("remove backup {backup}: repository file write failed: injected unlink failure");
    assert_eq!(format!("{:#}", written.unwrap_err()), expected);
    assert_eq!(phase, "state_installed");
    assert_eq!(published(root), (AFTER.to_vec(), AFTER.to_vec()));
    assert_eq!(std::fs::read(&backup).unwrap(), BEFORE);
    assert_eq!(format!("{:#}", entered.unwrap_err()), expected);
    assert!(!ran, "the operation does not run");

    with_repository_publication(&layout, |_| Ok(())).unwrap();

    assert_eq!(backups(root), Vec::<String>::new());
    assert!(!layout.source_edit_marker_path().exists());
    assert_eq!(
        names(&layout.source_edit_transactions_dir()),
        Vec::<String>::new()
    );
    assert_eq!(published(root), (AFTER.to_vec(), AFTER.to_vec()));
}

/// The child stops after the rename of the prepared file and before the
/// removal of the backup, and then a person writes other bytes to the
/// backup. The section entry opens and completes the install, and the
/// backup stays.
#[test]
#[verifies("rule_stopped_write_leaves_no_transaction_data", examples)]
fn a_backup_that_a_person_changed_stays_and_the_section_entry_completes_the_install() {
    let temp = fixture();
    let root_path = canonical_root(&temp);
    let root = root_path.as_path();
    let layout = ProvenanceLayout::new(root);
    stop_in_child(CHILD, root, "repository_file_unlink_backup");
    let (backup, _) = marked_backup(&layout);
    std::fs::write(&backup, b"changed\n").unwrap();

    with_repository_publication(&layout, |_| Ok(())).unwrap();

    assert_eq!(std::fs::read(&backup).unwrap(), b"changed\n");
    assert!(!layout.source_edit_marker_path().exists());
    assert_eq!(
        names(&layout.source_edit_transactions_dir()),
        Vec::<String>::new()
    );
    assert_eq!(published(root), (AFTER.to_vec(), AFTER.to_vec()));
}

/// After a failed removal of the backup, a person changes the target. The
/// next section entry does not open, and its error names the target and the
/// marker.
#[test]
#[verifies("rule_failed_removal_of_stopped_write_names_its_path", examples)]
fn a_person_change_after_a_failed_removal_of_the_backup_stops_the_section_entry_with_the_paths() {
    let temp = fixture();
    let root_path = canonical_root(&temp);
    let root = root_path.as_path();
    let layout = ProvenanceLayout::new(root);
    test_probes::arm("repository_file_unlink_backup", || {
        anyhow::bail!("injected unlink failure")
    });
    try_publish(root).unwrap_err();
    test_probes::disarm("repository_file_unlink_backup");
    std::fs::write(root.join("source.txt"), b"person\n").unwrap();

    let mut ran = false;
    let entered = with_repository_publication(&layout, |_| {
        ran = true;
        Ok(())
    });

    let error = crate::write_error::WriteError(entered.unwrap_err());
    assert_eq!(
        format!("{:#}", error.0),
        format!(
            "end the write that {} names: source file {} changed outside the pending \
             source-edit transaction",
            layout.source_edit_marker_path(),
            root.join("source.txt")
        )
    );
    assert!(matches!(
        error.safe(),
        crate::write_error::WriteFailure::WriteFailed { .. }
    ));
    assert!(!ran, "the operation does not run");
    assert_eq!(std::fs::read(root.join("source.txt")).unwrap(), b"person\n");
}

/// The store renames the target to the backup, a later step before the
/// rename of the prepared file fails, and the rename back fails too. The
/// write keeps its marker and names the backup. The next section entry puts
/// the backup back and completes the install.
#[test]
#[verifies("rule_stopped_write_leaves_no_transaction_data", examples)]
fn a_failed_restore_keeps_the_marker_until_the_section_entry_completes_the_install() {
    let temp = fixture();
    let root_path = canonical_root(&temp);
    let root = root_path.as_path();
    let layout = ProvenanceLayout::new(root);
    test_probes::arm("repository_file_after_backup_check", || {
        anyhow::bail!("injected install failure")
    });
    test_probes::arm("repository_file_restore", || {
        anyhow::bail!("injected restore failure")
    });

    let written = try_publish(root);
    test_probes::disarm("repository_file_after_backup_check");
    test_probes::disarm("repository_file_restore");

    let (backup, phase) = marked_backup(&layout);
    assert_eq!(
        format!("{:#}", written.unwrap_err()),
        format!("restore backup {backup}: injected restore failure")
    );
    assert_eq!(phase, "state_installed");
    assert!(!root.join("source.txt").exists());
    assert_eq!(std::fs::read(&backup).unwrap(), BEFORE);

    with_repository_publication(&layout, |_| Ok(())).unwrap();

    assert_eq!(published(root), (AFTER.to_vec(), AFTER.to_vec()));
    assert_eq!(backups(root), Vec::<String>::new());
    assert_eq!(
        files_named(root, ".source.txt.provenance-", ".tmp"),
        Vec::<String>::new()
    );
    assert!(!layout.source_edit_marker_path().exists());
    assert_eq!(
        names(&layout.source_edit_transactions_dir()),
        Vec::<String>::new()
    );
}
