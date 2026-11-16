//! A write of one state key that stops between the sync of its temporary
//! file and its rename, and a section entry while such a write waits at that
//! point.
//!
//! The state directory is in git, so a file that a stopped write leaves there
//! shows as an untracked file. These tests compare the files below the state
//! directory before and after the write.

use super::stage_recovery_tests::{scope, serial, stamp, write_one_rule, STAGED_RULE};
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

    assert_eq!(state_files(&root), state_files_of(&old), "state files");
    assert!(state_tree(&root) == old, "the stop kept the old bytes");
    assert_eq!(cache_temporaries(&layout).len(), 1, "temporary file");

    let repository = open(&layout);
    assert_eq!(staged_rules(&repository), 0);
    assert_eq!(cache_temporaries(&layout), Vec::<String>::new());
    assert_eq!(serial(&repository), before, "serial");

    repository
        .stage(false, Some(stamp()), write_one_rule)
        .unwrap();
    assert_eq!(staged_rules(&repository), 1, "a later write lands once");
    assert_eq!(state_files(&root), state_files_of(&old), "state files");
    assert_eq!(cache_temporaries(&layout), Vec::<String>::new());
}

/// While a write of one key waits between the sync of its temporary file and
/// its rename, a second writer that enters the section waits for the lock.
/// Its clean-up thus cannot remove the temporary file of the live write, and
/// both calls succeed.
#[test]
#[verifies("rule_one_key_write_adds_no_other_state_file", examples)]
#[verifies("rule_stopped_write_leaves_no_transaction_data", examples)]
fn a_section_entry_waits_for_a_one_key_write_at_its_rename() {
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
        assert_eq!(state_files(&root), old, "state files of the live write");
        assert_eq!(cache_temporaries(layout).len(), 1, "temporary file");

        let second = threads.spawn(move || {
            let count = staged_rules(&open(layout));
            done_tx.send(()).unwrap();
            count
        });
        assert!(done_rx.recv_timeout(Duration::from_millis(300)).is_err());
        assert_eq!(
            cache_temporaries(layout).len(),
            1,
            "the live write keeps its file"
        );

        release_tx.send(()).unwrap();
        first.join().unwrap().unwrap();
        assert_eq!(
            second.join().unwrap(),
            1,
            "the second writer reads the write"
        );
    });
    assert_eq!(staged_rules(&open(&layout)), 1);
    assert_eq!(state_files(&root), old, "state files");
    assert_eq!(cache_temporaries(&layout), Vec::<String>::new());
}

fn state_files_of(tree: &std::collections::BTreeMap<Utf8PathBuf, Vec<u8>>) -> Vec<Utf8PathBuf> {
    tree.keys().cloned().collect()
}
