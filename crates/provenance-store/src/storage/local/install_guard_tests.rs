//! The checks of an install in place: a staged file that the change list
//! does not account for, a link in the path of a live key, a cache on a
//! different file system, and the cause of a failed replace on Windows.

use super::in_place_tests::{enter, live, read, stopped_write, QUESTIONS, REQUIREMENTS, RULES};
use super::LocalStateStorage;
use crate::publication::with_repository_publication;
use crate::storage::{Atomicity, PublishFailure, StateKey, StateStorage, WriteSet};
use provenance_macros::verifies;

/// The staged file of `REQUIREMENTS` is gone, and its live file does not
/// have the bytes of the put. The section entry refuses with the path of
/// the staged file, applies no delete, and keeps the marker.
#[test]
#[verifies("rule_stopped_write_completes_after_its_marker", examples)]
fn the_next_entry_refuses_a_put_whose_staged_file_is_gone_and_not_installed() {
    let (_temp, layout) = live();
    let transaction = stopped_write(&layout, &[RULES, REQUIREMENTS], &[RULES], &[QUESTIONS]);

    let error = format!("{:#}", enter(&layout).unwrap_err());

    let missing = transaction.join("staged").join(REQUIREMENTS);
    let live = layout.state_dir().join(REQUIREMENTS);
    assert_eq!(
        error,
        format!(
            "staged file {missing} is missing, and {live} does not have the bytes that the \
             change list names for {REQUIREMENTS}"
        )
    );
    assert_eq!(read(&layout, QUESTIONS).as_deref(), Some("old"));
    assert!(layout.publication_marker_path().exists());
}

/// The staged file of `RULES` has bytes that the change list does not name.
/// The section entry refuses with its path and installs nothing.
#[test]
#[verifies("rule_stopped_write_completes_after_its_marker", examples)]
fn the_next_entry_refuses_a_staged_file_with_other_bytes() {
    let (_temp, layout) = live();
    let transaction = stopped_write(&layout, &[RULES], &[RULES], &[]);
    let staged = transaction.join("staged").join(RULES);
    std::fs::write(&staged, "changed").unwrap();

    let error = format!("{:#}", enter(&layout).unwrap_err());

    assert_eq!(
        error,
        format!(
            "staged file {staged} does not have the bytes that the change list names for \
             {RULES}"
        )
    );
    assert_eq!(read(&layout, RULES).as_deref(), Some("old"));
    assert!(layout.publication_marker_path().exists());
}

/// A link in place of the `rules` directory of the live state points to a
/// directory outside the repository. The recovery refuses the path and
/// writes nothing through the link.
#[cfg(unix)]
#[test]
#[verifies("rule_stopped_write_completes_after_its_marker", examples)]
fn the_recovery_writes_nothing_through_a_linked_state_directory() {
    let (_temp, layout) = live();
    let outside = tempfile::tempdir().unwrap();
    let rules = layout.state_dir().join("scopes/default/rules");
    std::fs::remove_dir_all(&rules).unwrap();
    std::os::unix::fs::symlink(outside.path(), &rules).unwrap();
    stopped_write(&layout, &[RULES], &[RULES], &[]);

    let error = format!("{:#}", enter(&layout).unwrap_err());

    assert_eq!(error, format!("unsupported state entry: {rules}"));
    assert_eq!(std::fs::read_dir(outside.path()).unwrap().count(), 0);
    assert!(layout.publication_marker_path().exists());
}

/// A whole set whose key is below a linked directory is refused before its
/// marker, and nothing is written through the link.
#[cfg(unix)]
#[test]
#[verifies("rule_write_passes_while_a_record_file_is_held", examples)]
fn a_whole_set_below_a_linked_state_directory_is_refused_before_its_marker() {
    let (_temp, layout) = live();
    let outside = tempfile::tempdir().unwrap();
    let rules = layout.state_dir().join("scopes/default/rules");
    std::fs::remove_dir_all(&rules).unwrap();
    std::os::unix::fs::symlink(outside.path(), &rules).unwrap();
    let writes = WriteSet::new(Atomicity::WholeSet)
        .put(StateKey::new(RULES).unwrap(), b"new".to_vec())
        .put(StateKey::new(QUESTIONS).unwrap(), b"new".to_vec());

    let landed = with_repository_publication(&layout, |_| {
        Ok(LocalStateStorage::new(layout.clone()).publish(writes))
    })
    .unwrap();

    let Err(PublishFailure::Refused(error)) = landed else {
        panic!("the set is refused: {landed:?}");
    };
    assert_eq!(
        format!("{error:#}"),
        format!("unsupported state entry: {rules}")
    );
    assert_eq!(std::fs::read_dir(outside.path()).unwrap().count(), 0);
    assert_eq!(read(&layout, QUESTIONS).as_deref(), Some("old"));
    assert!(!layout.publication_marker_path().exists());
}

/// `/proc` is on a different file system than a temporary directory, as a
/// cache on a different mount is. The check refuses the install.
#[cfg(target_os = "linux")]
#[test]
fn the_install_refuses_a_cache_on_a_different_file_system() {
    let temp = tempfile::tempdir().unwrap();
    let state = camino::Utf8Path::from_path(temp.path()).unwrap();
    let cache = camino::Utf8Path::new("/proc");

    let error = super::live_entry::ensure_one_file_system(cache, state).unwrap_err();

    assert_eq!(
        format!("{error:#}"),
        format!(
            "install the state from the cache {cache} in {state}: the cache and the state \
             directory are on two file systems, and the store writes the state only when they \
             are on one file system"
        )
    );
    super::live_entry::ensure_one_file_system(state, state).unwrap();
}

/// On Windows, an access denial is a held file only when a different
/// handle has the file open with no delete sharing. A read-only attribute
/// also gives an access denial on a replace, but it is not a held file.
#[cfg(windows)]
#[test]
#[verifies("rule_held_record_file_refusal_names_the_file", examples)]
fn an_access_denial_is_a_held_file_only_while_a_holder_has_the_file_open() {
    use super::live_entry::held_by_another_process;
    use std::os::windows::fs::OpenOptionsExt;
    const FILE_SHARE_READ: u32 = 0x1;
    let (_temp, layout) = live();
    let path = layout.state_dir().join(RULES);
    let set_read_only = |read_only: bool| {
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        permissions.set_readonly(read_only);
        std::fs::set_permissions(&path, permissions).unwrap();
    };

    set_read_only(true);
    let read_only = held_by_another_process(&path, 5);
    set_read_only(false);
    let holder = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .open(&path)
        .unwrap();
    let held = (
        held_by_another_process(&path, 5),
        held_by_another_process(&path, 32),
    );
    drop(holder);

    assert!(!read_only);
    assert_eq!(held, (true, true));
    assert!(!held_by_another_process(&path, 5));
}
