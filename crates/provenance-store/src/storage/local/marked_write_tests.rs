//! A whole set whose install stops or fails after its marker, seen through
//! the storage port of the local adapter, which has no section view. No
//! read in the section sees the set in part, the next section entry
//! completes it, and it is never left in part.

use super::LocalStateStorage;
use crate::layout::ProvenanceLayout;
use crate::publication::with_repository_publication;
use crate::stopped_child::{run_as_child, stop_in_child};
use crate::storage::{Atomicity, StateKey, StateStorage, WriteSet};
use crate::test_probes;
use crate::test_support::directory::utf8;
use provenance_macros::verifies;
use std::collections::BTreeMap;

const KEYS: [&str; 3] = [
    "scopes/a/questions/question.jsonl",
    "scopes/a/rules/rule.jsonl",
    "scopes/a/sources/source.jsonl",
];

/// A layout with each key at `old`.
fn live(temp: &tempfile::TempDir) -> ProvenanceLayout {
    let layout = ProvenanceLayout::new(utf8(&dunce::canonicalize(temp.path()).unwrap()));
    for key in KEYS {
        let path = layout.state_dir().join(key);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "old").unwrap();
    }
    layout
}

/// The whole set that puts `new` in each key.
fn new_set() -> WriteSet {
    KEYS.iter()
        .fold(WriteSet::new(Atomicity::WholeSet), |set, key| {
            set.put(StateKey::new(*key).unwrap(), b"new".to_vec())
        })
}

fn state(bytes: &str) -> BTreeMap<StateKey, Vec<u8>> {
    KEYS.iter()
        .map(|key| (StateKey::new(*key).unwrap(), bytes.as_bytes().to_vec()))
        .collect()
}

/// The bytes of each key on the disk.
fn on_disk(layout: &ProvenanceLayout) -> Vec<String> {
    KEYS.iter()
        .map(|key| std::fs::read_to_string(layout.state_dir().join(key)).unwrap())
        .collect()
}

/// The view that the port reads in a new section.
fn next_section_view(layout: &ProvenanceLayout) -> BTreeMap<StateKey, Vec<u8>> {
    with_repository_publication(layout, |_| {
        Ok(LocalStateStorage::new(layout.clone()).read_view()?)
    })
    .unwrap()
    .into_entries()
}

/// The install fails one time after the first key. The write succeeds, and
/// the disk has one new key and two old keys. A read in the same section
/// completes the install first, so it sees the whole set.
#[test]
#[verifies("rule_section_reads_no_partial_whole_set", examples)]
#[verifies("rule_write_succeeds_after_its_marker", examples)]
fn a_read_in_the_section_completes_an_install_that_failed_first() {
    let temp = tempfile::tempdir().unwrap();
    let layout = live(&temp);
    let storage = LocalStateStorage::new(layout.clone());
    let mut failures = 1;
    test_probes::arm("state_file_installed", move || {
        if failures == 0 {
            return Ok(());
        }
        failures -= 1;
        anyhow::bail!("injected install failure")
    });

    let (disk, seen) = with_repository_publication(&layout, |_| {
        storage.publish(new_set()).unwrap();
        let disk = on_disk(&layout);
        Ok((disk, storage.read_view().map(|view| view.into_entries())))
    })
    .unwrap();
    test_probes::disarm("state_file_installed");

    assert_eq!(disk, ["new", "old", "old"]);
    assert_eq!(seen.unwrap(), state("new"));
    assert!(!layout.publication_marker_path().exists());
    assert_eq!(on_disk(&layout), ["new", "new", "new"]);
}

/// The install fails after each key that it installs. Each read and each
/// write in the same section runs the install again and fails with its
/// error, so no read gives a view. When the cause is gone, the next section
/// entry completes the set.
#[test]
#[verifies("rule_section_reads_no_partial_whole_set", examples)]
fn a_read_in_the_section_fails_while_the_install_fails_and_the_next_entry_completes_it() {
    let temp = tempfile::tempdir().unwrap();
    let layout = live(&temp);
    let storage = LocalStateStorage::new(layout.clone());
    test_probes::arm("state_file_installed", || {
        anyhow::bail!("injected install failure")
    });

    let (seen, next_write) = with_repository_publication(&layout, |_| {
        storage.publish(new_set()).unwrap();
        let seen = storage.read_view().map(|view| view.into_entries());
        let other = WriteSet::new(Atomicity::PerKey)
            .put(StateKey::new(KEYS[1]).unwrap(), b"other".to_vec());
        Ok((seen, storage.publish(other)))
    })
    .unwrap();
    test_probes::disarm("state_file_installed");

    let error = format!("{:#}", seen.unwrap_err());
    assert!(error.contains("injected install failure"), "{error}");
    assert!(next_write.is_err());
    assert!(layout.publication_marker_path().exists());

    assert_eq!(next_section_view(&layout), state("new"));
    assert!(!layout.publication_marker_path().exists());
}

const CHILD: &str = "storage::local::marked_write_tests::stopped_write_child";

#[test]
fn stopped_write_child() {
    run_as_child(&["state_file_installed"], |root| {
        let layout = ProvenanceLayout::new(root);
        with_repository_publication(&layout, |_| {
            Ok(LocalStateStorage::new(layout.clone()).publish(new_set()))
        })
        .unwrap()
        .unwrap();
    });
}

/// The process stops after the first key of the install. The next section
/// entry completes the set before the port reads it.
#[test]
#[verifies("rule_stopped_write_completes_after_its_marker", examples)]
fn the_next_section_entry_completes_a_set_that_stopped_after_its_first_key() {
    let temp = tempfile::tempdir().unwrap();
    let layout = live(&temp);
    stop_in_child(CHILD, layout.root(), "state_file_installed");
    assert_eq!(on_disk(&layout), ["new", "old", "old"]);
    assert!(layout.publication_marker_path().exists());

    assert_eq!(next_section_view(&layout), state("new"));
    assert!(!layout.publication_marker_path().exists());
}
