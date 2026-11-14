//! The prepared file that a source edit makes next to its target: the state
//! that a stop at each point of its write leaves, and the next section entry.

use super::source_edit_recovery_tests::{
    canonical_root, fixture, prepared_files, published, AFTER, BEFORE, CHILD,
};
use crate::{
    layout::ProvenanceLayout,
    operations::files::RepositoryFiles,
    publication::{with_repository_publication, with_staged_state_and_source_edit},
    stopped_child::{names, run_as_child, stop_in_child},
};
use camino::Utf8Path;
use provenance_macros::verifies;

/// A replacement that is longer than the record of the prepared file.
const LONG: [u8; 4096] = [b'x'; 4096];

/// The test that runs a source edit with the replacement [`LONG`] as a child.
#[cfg(unix)]
const LONG_CHILD: &str = "operations::files::tests::prepared_file_tests::long_child";

#[test]
fn long_child() {
    run_as_child(&["state_prepared"], |root| {
        let held = RepositoryFiles::open(root)
            .unwrap()
            .read_bounded(Utf8Path::new("source.txt"), 1024)
            .unwrap();
        with_repository_publication(&ProvenanceLayout::new(root), |section| {
            with_staged_state_and_source_edit(section, held, &LONG, |_| Ok(()))
        })
        .unwrap();
    });
}

/// Asserts that a section entry opens, and that no transaction, prepared
/// file or marker of the source edit stays, and that the file and the state
/// are the old ones.
fn assert_entry_removes_the_stopped_edit(root: &Utf8Path) {
    let layout = ProvenanceLayout::new(root);
    with_repository_publication(&layout, |_| Ok(())).unwrap();
    assert_eq!(
        names(&layout.source_edit_transactions_dir()),
        Vec::<String>::new()
    );
    assert_eq!(prepared_files(root), Vec::<String>::new());
    assert!(!layout.source_edit_marker_path().exists());
    assert_eq!(published(root), (BEFORE.to_vec(), BEFORE.to_vec()));
}

/// The child stops in the write of the replacement in its transaction: after
/// the probe before that write, the system ends the child at the first byte
/// past the length of the record of the prepared file. The record names the
/// prepared file, and the file does not exist.
#[cfg(unix)]
#[test]
#[verifies("rule_stopped_write_leaves_no_transaction_data", examples)]
fn a_stop_before_the_prepared_file_exists_leaves_no_data_after_the_next_section_entry() {
    let temp = fixture();
    let root_path = canonical_root(&temp);
    let root = root_path.as_path();
    crate::stopped_child::stop_in_child_at_file_size(LONG_CHILD, root, "state_prepared", 1024);
    let transactions = ProvenanceLayout::new(root).source_edit_transactions_dir();
    assert_eq!(names(&transactions).len(), 1, "the stop left its transaction");
    assert_eq!(prepared_files(root), Vec::<String>::new());

    assert_entry_removes_the_stopped_edit(root);
}

/// The child stops at the first byte that it writes after it made and synced
/// the prepared file.
#[cfg(unix)]
#[test]
#[verifies("rule_stopped_write_leaves_no_transaction_data", examples)]
fn a_stop_after_the_prepared_file_is_made_leaves_no_data_after_the_next_section_entry() {
    let temp = fixture();
    let root_path = canonical_root(&temp);
    let root = root_path.as_path();
    crate::stopped_child::stop_in_child_at_file_size(
        CHILD,
        root,
        "repository_file_clone_parent",
        0,
    );
    let prepared = prepared_files(root);
    assert_eq!(prepared.len(), 1, "the stop left the prepared file");
    assert_eq!(std::fs::read(root.join(&prepared[0])).unwrap(), AFTER);

    assert_entry_removes_the_stopped_edit(root);
}

/// Cuts the prepared file of the source edit at `root` to its first `length`
/// bytes in place, as a stop in its write leaves it.
fn cut_prepared_file(root: &Utf8Path, length: u64) {
    let prepared = prepared_files(root);
    assert_eq!(prepared.len(), 1);
    std::fs::OpenOptions::new()
        .write(true)
        .open(root.join(&prepared[0]))
        .unwrap()
        .set_len(length)
        .unwrap();
}

/// No probe stops the child in the write of the prepared file, so the child
/// stops after that write, and the test cuts the file to a leading part of
/// the replacement, the empty part included.
#[test]
#[verifies("rule_stopped_write_leaves_no_transaction_data", examples)]
fn a_prepared_file_with_a_leading_part_of_the_replacement_is_removed_at_the_next_section_entry() {
    for length in [0, 3] {
        let temp = fixture();
        let root_path = canonical_root(&temp);
        let root = root_path.as_path();
        stop_in_child(CHILD, root, "source_edit_temp_prepared");
        cut_prepared_file(root, length);

        assert_entry_removes_the_stopped_edit(root);
    }
}

/// A marked source edit finds the prepared file before it installs the
/// file. A prepared file with a leading part of the replacement is the
/// store's own data: the recovery removes it and installs the replacement.
#[test]
#[verifies("rule_stopped_write_leaves_no_transaction_data", examples)]
fn a_marked_source_edit_replaces_a_prepared_file_with_a_leading_part_of_the_replacement() {
    for phase in [
        "source_edit_prepared",
        "source_edit_backup_created",
        "source_edit_state_installed",
    ] {
        let temp = fixture();
        let root_path = canonical_root(&temp);
        let root = root_path.as_path();
        let layout = ProvenanceLayout::new(root);
        stop_in_child(CHILD, root, phase);
        cut_prepared_file(root, 3);

        with_repository_publication(&layout, |_| Ok(())).unwrap();

        assert_eq!(published(root), (AFTER.to_vec(), AFTER.to_vec()), "{phase}");
        assert_eq!(prepared_files(root), Vec::<String>::new(), "{phase}");
        assert!(!layout.source_edit_marker_path().exists(), "{phase}");
        assert_eq!(
            names(&layout.source_edit_transactions_dir()),
            Vec::<String>::new(),
            "{phase}"
        );
    }
}

/// A prepared file with content that is not a leading part of the
/// replacement is a change that a person made. The section entry fails with
/// the path of the file, the operation does not run, and the file and the
/// target stay, for a stopped write with no marker and for a marked write.
#[test]
#[verifies("rule_failed_removal_of_stopped_write_names_its_path", examples)]
fn a_prepared_file_that_a_person_changed_stops_the_section_entry_and_stays() {
    for phase in ["source_edit_temp_prepared", "source_edit_prepared"] {
        let temp = fixture();
        let root_path = canonical_root(&temp);
        let root = root_path.as_path();
        let layout = ProvenanceLayout::new(root);
        stop_in_child(CHILD, root, phase);
        let prepared = root.join(&prepared_files(root)[0]);
        std::fs::write(&prepared, b"changed\n").unwrap();

        let mut ran = false;
        let error = with_repository_publication(&layout, |_| {
            ran = true;
            Ok(())
        })
        .unwrap_err();

        assert_eq!(
            format!("{error:#}"),
            format!("remove prepared file {prepared}: repository file changed after it was read"),
            "{phase}"
        );
        assert!(!ran, "{phase}");
        assert_eq!(std::fs::read(&prepared).unwrap(), b"changed\n", "{phase}");
        assert_eq!(std::fs::read(root.join("source.txt")).unwrap(), BEFORE, "{phase}");
        assert_eq!(
            layout.source_edit_marker_path().exists(),
            phase == "source_edit_prepared",
            "{phase}: the marker of a marked write stays"
        );
    }
}
