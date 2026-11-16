//! A write of one state key that stops between the sync of its temporary
//! file and its rename, and a second writer while such a write waits at that
//! point.
//!
//! The state directory is in git, so a file that a stopped write leaves there
//! shows as an untracked file. These tests compare the files below the state
//! directory before and after the write.

use super::stage_recovery_tests::{
    scope, serial, stamp, transactions, write_one_requirement, write_one_rule, STAGED_REQUIREMENT,
    STAGED_RULE,
};
use super::Repository;
use crate::cache::tests::fixtures::{open, seeded_layout};
use crate::layout::ProvenanceLayout;
use crate::stopped_child::{names, run_as_child, state_tree};
use crate::test_probes;
use camino::Utf8PathBuf;
use provenance_macros::verifies;
use std::sync::mpsc;
use std::time::Duration;

/// The probe between the sync of the temporary file of a key and its rename.
const PROBE: &str = "synced_file_before_rename";

/// The test that the stop test runs as a child.
const CHILD: &str = "repository::one_key_stop_tests::one_key_child";

#[test]
fn one_key_child() {
    run_as_child(&[PROBE], |root| {
        let repository = open(&ProvenanceLayout::new(root.to_owned()));
        repository
            .stage(false, Some(stamp()), write_one_rule)
            .unwrap();
    });
}

/// The path of each file below the state directory under `root`, relative
/// to the state directory.
fn state_files(root: &camino::Utf8Path) -> Vec<Utf8PathBuf> {
    state_tree(root).into_keys().collect()
}

/// The names of the regular files directly in the cache that end with
/// `.tmp`: the temporary files of the store in the cache.
fn cache_temporaries(layout: &ProvenanceLayout) -> Vec<String> {
    let cache = layout.cache_dir();
    names(&cache)
        .into_iter()
        .filter(|name| {
            std::path::Path::new(name).extension() == Some("tmp".as_ref())
                && cache.join(name).is_file()
        })
        .collect()
}

/// The number of Rules with the id of the staged Rule. The read enters the
/// publication section.
fn staged_rules(repository: &Repository) -> usize {
    let rules = repository.store().list_rules(&scope()).unwrap();
    rules
        .iter()
        .filter(|rule| rule.id.as_str() == STAGED_RULE)
        .count()
}

/// A stop between the synced temporary file of the key and its rename keeps
/// the old bytes of the key and adds no file below the state directory. The
/// temporary file stays in the cache until the next section entry removes
/// it, and the write then lands one time.
#[test]
#[verifies("rule_one_key_write_set_lands_whole", examples)]
#[verifies("rule_one_key_write_adds_no_other_state_file", examples)]
#[verifies("rule_stopped_write_leaves_no_transaction_data", examples)]
fn a_one_key_write_stopped_before_its_rename_leaves_no_file_in_the_state_directory() {
    let (_dir, layout, _scope) = seeded_layout();
    let root = layout.root().to_owned();
    let before = serial(&open(&layout));
    let old = state_tree(&root);
    crate::stopped_child::stop_in_child(CHILD, &root, PROBE);

    assert!(
        state_tree(&root) == old,
        "the stop kept the files and bytes"
    );
    assert_eq!(cache_temporaries(&layout).len(), 1, "temporary file");

    let repository = open(&layout);
    assert_eq!(staged_rules(&repository), 0);
    assert!(!layout.publication_marker_path().exists(), "marker");
    assert_eq!(transactions(&root), Vec::<Utf8PathBuf>::new());
    assert_eq!(cache_temporaries(&layout), Vec::<String>::new());
    assert_eq!(serial(&repository), before, "serial");

    repository
        .stage(false, Some(stamp()), write_one_rule)
        .unwrap();
    assert_eq!(staged_rules(&repository), 1, "a later write lands once");
    assert_eq!(state_files(&root), state_files_of(&old), "state files");
    assert_eq!(cache_temporaries(&layout), Vec::<String>::new());
}

/// The Requirements with the id of the staged Requirement.
fn staged_requirements(repository: &Repository) -> usize {
    let requirements = repository.store().list_requirements(&scope()).unwrap();
    requirements
        .iter()
        .filter(|requirement| requirement.id.as_str() == STAGED_REQUIREMENT)
        .count()
}

// No Rule: no Rule states that a writer waits for the publication section of
// another writer. The test shows that the removal of the data of a stopped
// write at the section entry cannot remove the temporary file of a live
// write of one key.
/// While a write of one key waits between the sync of its temporary file and
/// its rename, a second writer of one key waits to enter the section. So the
/// removal of stopped data at its entry does not remove the temporary file of
/// the live write, and both writes land.
#[test]
fn a_second_writer_does_not_remove_the_temporary_file_of_a_live_one_key_write() {
    let (_dir, layout, _scope) = seeded_layout();
    let root = layout.root().to_owned();
    let old = state_files(&root);
    std::thread::scope(|threads| {
        // The channels drop with this closure, so a failed assertion also
        // releases the first writer.
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let (done_tx, done_rx) = mpsc::channel();
        let layout = &layout;
        let first = threads.spawn(move || {
            let mut gate = Some((entered_tx, release_rx));
            test_probes::arm(PROBE, move || {
                if let Some((entered, release)) = gate.take() {
                    entered.send(()).unwrap();
                    let _ = release.recv();
                }
                Ok(())
            });
            let landed = open(layout).stage(false, Some(stamp()), write_one_rule);
            test_probes::disarm(PROBE);
            landed
        });
        entered_rx.recv_timeout(Duration::from_secs(20)).unwrap();
        assert_eq!(cache_temporaries(layout).len(), 1, "temporary file");

        let second = threads.spawn(move || {
            let landed = open(layout).stage(false, Some(stamp()), write_one_requirement);
            done_tx.send(()).unwrap();
            landed
        });
        assert!(
            done_rx.recv_timeout(Duration::from_millis(300)).is_err(),
            "the second writer did not wait for the section"
        );
        assert_eq!(
            cache_temporaries(layout).len(),
            1,
            "the live write keeps its temporary file"
        );

        release_tx.send(()).unwrap();
        first.join().unwrap().unwrap();
        second.join().unwrap().unwrap();
    });
    let repository = open(&layout);
    assert_eq!(staged_rules(&repository), 1, "the first write landed");
    assert_eq!(
        staged_requirements(&repository),
        1,
        "the second write landed"
    );
    assert_eq!(state_files(&root), old, "state files");
    assert_eq!(cache_temporaries(&layout), Vec::<String>::new());
}

fn state_files_of(tree: &std::collections::BTreeMap<Utf8PathBuf, Vec<u8>>) -> Vec<Utf8PathBuf> {
    tree.keys().cloned().collect()
}
