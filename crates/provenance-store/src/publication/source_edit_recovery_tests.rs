use crate::stopped_child::{files_named, names, run_as_child, stop_in_child};
use crate::{
    layout::ProvenanceLayout,
    operations::files::RepositoryFiles,
    publication::{
        with_repository_publication, with_staged_state_and_source_edit, SourceEditRecoveryFailure,
    },
    test_probes,
};
use camino::Utf8Path;
use provenance_macros::verifies;

pub(super) const BEFORE: &[u8] = b"before\n";
pub(super) const AFTER: &[u8] = b"after\n";

pub(super) fn canonical_root(temp: &tempfile::TempDir) -> camino::Utf8PathBuf {
    camino::Utf8PathBuf::from_path_buf(dunce::canonicalize(temp.path()).unwrap()).unwrap()
}

pub(super) fn fixture() -> tempfile::TempDir {
    let temp = tempfile::tempdir().unwrap();
    let root_path = canonical_root(&temp);
    let root = root_path.as_path();
    let layout = ProvenanceLayout::new(root);
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    std::fs::write(layout.state_dir().join("value"), BEFORE).unwrap();
    std::fs::write(root.join("source.txt"), BEFORE).unwrap();
    temp
}

pub(super) fn try_publish(root: &Utf8Path) -> anyhow::Result<()> {
    let layout = ProvenanceLayout::new(root);
    let files = RepositoryFiles::open(root).unwrap();
    let held = files
        .read_bounded(Utf8Path::new("source.txt"), 1024)
        .unwrap();
    with_repository_publication(&layout, |section| {
        with_staged_state_and_source_edit(section, held, AFTER, |staged| {
            staged.publish_shard(&staged.layout.state_dir().join("value"), AFTER.to_vec())
        })
    })
}

pub(super) fn publish(root: &Utf8Path) {
    try_publish(root).unwrap();
}

/// The source file and the state value of the fixture at `root`.
pub(super) fn published(root: &Utf8Path) -> (Vec<u8>, Vec<u8>) {
    let layout = ProvenanceLayout::new(root);
    (
        std::fs::read(root.join("source.txt")).unwrap(),
        std::fs::read(layout.state_dir().join("value")).unwrap(),
    )
}

/// The test that the stop tests run as a child.
pub(super) const CHILD: &str = "operations::files::tests::source_edit_recovery_tests::crash_child";

#[test]
fn crash_child() {
    run_as_child(
        &[
            "state_prepared",
            "repository_file_clone_parent",
            "source_edit_temp_prepared",
            "source_edit_prepared",
            "source_edit_backup_created",
            "source_edit_state_installed",
            "source_edit_file_installed",
        ],
        publish,
    );
}

#[test]
fn each_source_edit_phase_recovers_matching_file_and_state() {
    for phase in [
        "source_edit_prepared",
        "source_edit_backup_created",
        "source_edit_state_installed",
        "source_edit_file_installed",
    ] {
        let temp = fixture();
        let root_path = canonical_root(&temp);
        let root = root_path.as_path();
        stop_in_child(CHILD, root, phase);

        let layout = ProvenanceLayout::new(root);
        with_repository_publication(&layout, |_| Ok(())).unwrap();
        assert_eq!(
            std::fs::read(root.join("source.txt")).unwrap(),
            AFTER,
            "{phase}"
        );
        assert_eq!(
            std::fs::read(layout.state_dir().join("value")).unwrap(),
            AFTER,
            "{phase}"
        );
        assert!(!layout.source_edit_marker_path().exists(), "{phase}");
        assert_eq!(prepared_files(root), Vec::<String>::new(), "{phase}");
    }
}

/// The names of the prepared files of a source edit next to the target.
pub(super) fn prepared_files(root: &Utf8Path) -> Vec<String> {
    files_named(root, ".source.txt.provenance-", ".tmp")
}

/// A stop after the copy of the state and before the record of the prepared
/// file, and a stop after the prepared file and before the marker.
#[test]
#[verifies("rule_stopped_write_leaves_no_transaction_data", examples)]
fn a_stopped_source_edit_leaves_no_transaction_after_the_next_section_entry() {
    for phase in ["state_prepared", "source_edit_temp_prepared"] {
        let temp = fixture();
        let root_path = canonical_root(&temp);
        let root = root_path.as_path();
        let layout = ProvenanceLayout::new(root);
        stop_in_child(CHILD, root, phase);
        assert_eq!(
            names(&layout.source_edit_transactions_dir()).len(),
            1,
            "{phase}: the stopped write left its transaction"
        );
        assert!(!layout.source_edit_marker_path().exists(), "{phase}");
        assert_eq!(
            prepared_files(root).len(),
            usize::from(phase == "source_edit_temp_prepared"),
            "{phase}: the stopped write left its prepared file"
        );

        with_repository_publication(&layout, |_| Ok(())).unwrap();

        assert_eq!(
            names(&layout.source_edit_transactions_dir()),
            Vec::<String>::new(),
            "{phase}: the cache keeps no transaction data"
        );
        assert_eq!(
            std::fs::read(root.join("source.txt")).unwrap(),
            BEFORE,
            "{phase}"
        );
        assert_eq!(
            std::fs::read(layout.state_dir().join("value")).unwrap(),
            BEFORE,
            "{phase}"
        );
        assert_eq!(prepared_files(root), Vec::<String>::new(), "{phase}");
    }
}

#[test]
fn recovery_refuses_third_file_bytes_without_overwriting_them() {
    let temp = fixture();
    let root_path = canonical_root(&temp);
    let root = root_path.as_path();
    stop_in_child(CHILD, root, "source_edit_prepared");
    std::fs::write(root.join("source.txt"), b"external\n").unwrap();
    let layout = ProvenanceLayout::new(root);

    let error = with_repository_publication(&layout, |_| Ok(())).unwrap_err();

    assert!(matches!(
        error.downcast_ref::<SourceEditRecoveryFailure>(),
        Some(SourceEditRecoveryFailure::ExternalChange)
    ));
    assert_eq!(
        std::fs::read(root.join("source.txt")).unwrap(),
        b"external\n"
    );
    assert_eq!(
        std::fs::read(layout.state_dir().join("value")).unwrap(),
        BEFORE
    );
    assert!(layout.source_edit_marker_path().exists());
}

#[test]
fn file_install_refusal_rolls_state_back_without_overwriting_external_bytes() {
    let temp = fixture();
    let root_path = canonical_root(&temp);
    let root = root_path.as_path();
    let source = root.join("source.txt");
    let changed = source.clone();
    test_probes::arm("source_edit_state_installed", move || {
        std::fs::write(&changed, b"external\n").unwrap();
        Ok(())
    });

    let result = std::panic::catch_unwind(|| publish(root));
    test_probes::disarm("source_edit_state_installed");

    assert!(result.is_err());
    let layout = ProvenanceLayout::new(root);
    assert_eq!(std::fs::read(source).unwrap(), b"external\n");
    assert_eq!(
        std::fs::read(layout.state_dir().join("value")).unwrap(),
        BEFORE
    );
    assert!(!layout.source_edit_marker_path().exists());
}

#[test]
fn final_marker_clears_when_cleanup_lost_its_result() {
    let temp = fixture();
    let root_path = canonical_root(&temp);
    let root = root_path.as_path();
    stop_in_child(CHILD, root, "source_edit_file_installed");
    let layout = ProvenanceLayout::new(root);
    let marker: serde_json::Value =
        serde_json::from_slice(&std::fs::read(layout.source_edit_marker_path()).unwrap()).unwrap();
    let transaction = Utf8Path::new(marker["transaction_dir"].as_str().unwrap());
    std::fs::remove_dir_all(transaction).unwrap();

    with_repository_publication(&layout, |_| Ok(())).unwrap();

    assert_eq!(std::fs::read(root.join("source.txt")).unwrap(), AFTER);
    assert_eq!(
        std::fs::read(layout.state_dir().join("value")).unwrap(),
        AFTER
    );
    assert!(!layout.source_edit_marker_path().exists());
}

#[test]
fn failed_rollback_retains_material_and_restart_finishes_the_edit() {
    let temp = fixture();
    let root_path = canonical_root(&temp);
    let root = root_path.as_path();
    test_probes::crash_at("repository_file_after_backup_check");
    test_probes::crash_at("state_before_rollback");

    let result = std::panic::catch_unwind(|| publish(root));
    test_probes::disarm("repository_file_after_backup_check");
    test_probes::disarm("state_before_rollback");

    assert!(result.is_err());
    let layout = ProvenanceLayout::new(root);
    assert!(layout.source_edit_marker_path().exists());
    with_repository_publication(&layout, |_| Ok(())).unwrap();
    assert_eq!(std::fs::read(root.join("source.txt")).unwrap(), AFTER);
    assert_eq!(
        std::fs::read(layout.state_dir().join("value")).unwrap(),
        AFTER
    );
    assert!(!layout.source_edit_marker_path().exists());
}

/// A file and a link in the source-edit transaction directory are entries
/// that no write makes, so the test puts them there itself.
#[cfg(unix)]
#[test]
#[verifies("rule_recovery_stays_in_cache", examples)]
fn a_file_and_a_link_in_the_source_edit_transactions_stay_and_the_section_opens() {
    let temp = fixture();
    let root_path = canonical_root(&temp);
    let root = root_path.as_path();
    let layout = ProvenanceLayout::new(root);
    let outside = root.join("outside");
    std::fs::create_dir(&outside).unwrap();
    std::fs::write(outside.join("sentinel"), "keep").unwrap();
    let transactions = layout.source_edit_transactions_dir();
    std::fs::create_dir_all(&transactions).unwrap();
    std::fs::write(transactions.join("file"), "keep").unwrap();
    std::os::unix::fs::symlink(&outside, transactions.join("link")).unwrap();

    with_repository_publication(&layout, |_| Ok(())).unwrap();

    assert_eq!(names(&transactions), ["file", "link"]);
    assert_eq!(
        std::fs::read_to_string(outside.join("sentinel")).unwrap(),
        "keep"
    );
}

/// The probe after the `FileInstalled` marker takes the write permission from
/// the source-edit transaction directory, so the removal of the transaction
/// removes its contents and then fails.
#[cfg(unix)]
#[test]
#[verifies("rule_published_write_succeeds_when_its_cleanup_fails", examples)]
fn a_published_source_edit_answers_success_when_the_removal_of_its_transaction_fails() {
    use std::os::unix::fs::PermissionsExt;
    let temp = fixture();
    let root_path = canonical_root(&temp);
    let root = root_path.as_path();
    let layout = ProvenanceLayout::new(root);
    let transactions = layout.source_edit_transactions_dir();
    let locked = transactions.clone();
    test_probes::arm("source_edit_file_installed", move || {
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o500))?;
        Ok(())
    });

    let answer = try_publish(root);
    test_probes::disarm("source_edit_file_installed");
    let left = names(&transactions);
    std::fs::set_permissions(&transactions, std::fs::Permissions::from_mode(0o755)).unwrap();

    answer.unwrap();
    assert_eq!(left.len(), 1, "the transaction stays");
    assert_eq!(names(&transactions.join(&left[0])), Vec::<String>::new());
    assert!(layout.source_edit_marker_path().exists());
    assert_eq!(published(root), (AFTER.to_vec(), AFTER.to_vec()));
    with_repository_publication(&layout, |_| Ok(())).unwrap();
    assert_eq!(names(&transactions), Vec::<String>::new());
    assert!(!layout.source_edit_marker_path().exists());
    assert_eq!(published(root), (AFTER.to_vec(), AFTER.to_vec()));
}

/// The write stops after its `FileInstalled` marker. With no write permission
/// on the source-edit transaction directory, the next section entry removes
/// the contents of the transaction and then fails to remove the transaction.
#[cfg(unix)]
#[test]
#[verifies("rule_stopped_write_leaves_no_transaction_data", examples)]
#[verifies("rule_failed_removal_of_stopped_write_names_its_path", examples)]
fn a_section_entry_ends_a_published_source_edit_whose_transaction_is_removed_in_part() {
    use std::os::unix::fs::PermissionsExt;
    let temp = fixture();
    let root_path = canonical_root(&temp);
    let root = root_path.as_path();
    let layout = ProvenanceLayout::new(root);
    stop_in_child(CHILD, root, "source_edit_file_installed");
    let transactions = layout.source_edit_transactions_dir();
    let transaction = transactions.join(&names(&transactions)[0]);

    std::fs::set_permissions(&transactions, std::fs::Permissions::from_mode(0o500)).unwrap();
    let failed = with_repository_publication(&layout, |_| Ok(()));
    std::fs::set_permissions(&transactions, std::fs::Permissions::from_mode(0o755)).unwrap();
    let error = crate::write_error::WriteError(failed.unwrap_err());
    assert!(matches!(
        error.safe(),
        crate::write_error::WriteFailure::WriteFailed { .. }
    ));
    assert_eq!(
        format!("{:#}", error.0),
        format!(
            "remove transaction directory {transaction}: {}",
            std::io::Error::from_raw_os_error(13)
        )
    );
    assert_eq!(
        names(&transaction),
        Vec::<String>::new(),
        "a part is removed"
    );
    assert!(layout.source_edit_marker_path().exists());
    assert_eq!(published(root), (AFTER.to_vec(), AFTER.to_vec()));

    with_repository_publication(&layout, |_| Ok(())).unwrap();

    assert_eq!(names(&transactions), Vec::<String>::new());
    assert!(!layout.source_edit_marker_path().exists());
    assert_eq!(published(root), (AFTER.to_vec(), AFTER.to_vec()));
}

/// The child stops in the write of the prepared marker: after the probe
/// before that write, the system ends the child at the first byte of the
/// temporary file of the marker.
#[cfg(target_os = "linux")]
#[test]
#[verifies("rule_stopped_write_leaves_no_transaction_data", examples)]
fn a_stop_in_the_write_of_the_source_edit_marker_leaves_no_file_after_the_next_section_entry() {
    let temp = fixture();
    let root_path = canonical_root(&temp);
    let root = root_path.as_path();
    let layout = ProvenanceLayout::new(root);
    crate::stopped_child::stop_in_child_at_file_size(CHILD, root, "source_edit_temp_prepared", 0);
    let marker_temporaries = || {
        files_named(
            &layout.cache_dir(),
            ".source-edit-publication.json.",
            ".tmp",
        )
    };
    assert_eq!(
        marker_temporaries().len(),
        1,
        "the stop left the temporary file of the marker"
    );
    assert!(!layout.source_edit_marker_path().exists());

    with_repository_publication(&layout, |_| Ok(())).unwrap();

    assert_eq!(marker_temporaries(), Vec::<String>::new());
    assert_eq!(
        names(&layout.source_edit_transactions_dir()),
        Vec::<String>::new()
    );
    assert_eq!(prepared_files(root), Vec::<String>::new());
    assert_eq!(published(root), (BEFORE.to_vec(), BEFORE.to_vec()));
}
