//! The checks of an install in place: a staged file that the change list
//! does not account for, a link in the state directory, a cache on a
//! different file system, and the cause of a failed replace on Windows.

use super::in_place_tests::{enter, live, read, stopped_write, QUESTIONS, REQUIREMENTS, RULES};
use super::live_entry::key_path;
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

    let missing = key_path(&transaction.join("staged"), REQUIREMENTS);
    let live = key_path(&layout.state_dir(), REQUIREMENTS);
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
    let staged = key_path(&transaction.join("staged"), RULES);
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

/// The second put of the change list has a staged file with bytes that the
/// change list does not name. The section entry refuses before it installs
/// a file, so the first put keeps its live file.
#[test]
#[verifies("rule_stopped_write_completes_after_its_marker", examples)]
fn a_refusal_at_the_second_put_leaves_the_first_put_as_it_was() {
    let (_temp, layout) = live();
    let transaction = stopped_write(&layout, &[QUESTIONS, RULES], &[QUESTIONS, RULES], &[]);
    let staged = key_path(&transaction.join("staged"), RULES);
    std::fs::write(&staged, "changed").unwrap();

    let error = format!("{:#}", enter(&layout).unwrap_err());

    assert!(
        error.starts_with(&format!("staged file {staged} ")),
        "{error}"
    );
    assert_eq!(read(&layout, QUESTIONS).as_deref(), Some("old"));
    assert!(key_path(&transaction.join("staged"), QUESTIONS).is_file());
    assert!(layout.publication_marker_path().exists());
}

/// A link to a directory outside the repository.
fn link_directory(target: &std::path::Path, link: &camino::Utf8Path) {
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, link).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_dir(target, link).unwrap();
}

/// A link in place of the `rules` directory of the live state points to a
/// directory outside the repository. The recovery refuses the path and
/// writes nothing through the link.
#[cfg(any(unix, windows))]
#[test]
#[verifies("rule_whole_set_refuses_a_link_in_the_state_directory", examples)]
fn the_recovery_writes_nothing_through_a_linked_state_directory() {
    let (_temp, layout) = live();
    let outside = tempfile::tempdir().unwrap();
    let rules = layout.state_dir().join("scopes/default/rules");
    std::fs::remove_dir_all(&rules).unwrap();
    link_directory(outside.path(), &rules);
    stopped_write(&layout, &[RULES], &[RULES], &[]);

    let error = format!("{:#}", enter(&layout).unwrap_err());

    assert_eq!(error, format!("unsupported state entry: {rules}"));
    assert_eq!(std::fs::read_dir(outside.path()).unwrap().count(), 0);
    assert!(layout.publication_marker_path().exists());
}

/// A whole set whose key is below a linked directory is refused before its
/// marker, and nothing is written through the link.
#[cfg(any(unix, windows))]
#[test]
#[verifies("rule_whole_set_refuses_a_link_in_the_state_directory", examples)]
fn a_whole_set_below_a_linked_state_directory_is_refused_before_its_marker() {
    let (_temp, layout) = live();
    let outside = tempfile::tempdir().unwrap();
    let rules = layout.state_dir().join("scopes/default/rules");
    std::fs::remove_dir_all(&rules).unwrap();
    link_directory(outside.path(), &rules);
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

/// A link in the state directory that no key of the set names is refused
/// too, before the marker, with the path of the link.
#[cfg(any(unix, windows))]
#[test]
#[verifies("rule_whole_set_refuses_a_link_in_the_state_directory", examples)]
fn a_whole_set_refuses_a_link_elsewhere_in_the_state_directory() {
    let (_temp, layout) = live();
    let outside = tempfile::tempdir().unwrap();
    let notes = layout.state_dir().join("scopes/default/notes");
    link_directory(outside.path(), &notes);
    let writes =
        WriteSet::new(Atomicity::WholeSet).put(StateKey::new(QUESTIONS).unwrap(), b"new".to_vec());

    let landed = with_repository_publication(&layout, |_| {
        Ok(LocalStateStorage::new(layout.clone()).publish(writes))
    })
    .unwrap();

    let Err(PublishFailure::Refused(error)) = landed else {
        panic!("the set is refused: {landed:?}");
    };
    assert_eq!(
        format!("{error:#}"),
        format!("unsupported state entry: {notes}")
    );
    assert_eq!(read(&layout, QUESTIONS).as_deref(), Some("old"));
    assert!(!layout.publication_marker_path().exists());
}

/// `/proc` is on a different file system than a temporary directory, as a
/// cache on a different mount is. The check refuses the install. This test
/// is an implementation aid with no Rule, and it calls the check directly:
/// a test cannot mount a second file system below a repository, so no
/// call through the storage port can reach the refusal.
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
             are on one file system. Put .provenance/cache on the file system of \
             .provenance/state, and then send the call again"
        )
    );
    super::live_entry::ensure_one_file_system(state, state).unwrap();
}

/// On Windows, a read-only attribute gives an access denial on a replace,
/// as a holder with no delete sharing does, but it is not a held file. The
/// write succeeds after its marker, and a read in the section refuses with
/// the error of the install, which is not a held-file refusal. The next
/// section entry with no read-only attribute completes the set.
#[cfg(windows)]
#[test]
#[verifies("rule_held_record_file_refusal_names_the_file", examples)]
fn a_read_only_record_file_is_not_refused_as_a_held_file() {
    let (_temp, layout) = live();
    let path = layout.state_dir().join(RULES);
    let set_read_only = |read_only: bool| {
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        permissions.set_readonly(read_only);
        std::fs::set_permissions(&path, permissions).unwrap();
    };
    let writes =
        WriteSet::new(Atomicity::WholeSet).put(StateKey::new(RULES).unwrap(), b"new".to_vec());

    set_read_only(true);
    let (landed, seen) = with_repository_publication(&layout, |_| {
        let storage = LocalStateStorage::new(layout.clone());
        Ok((storage.publish(writes), storage.read_view()))
    })
    .unwrap();
    set_read_only(false);

    landed.unwrap();
    let Err(crate::storage::StorageError::Read(chain)) = seen else {
        panic!("the read refuses: {seen:?}");
    };
    assert!(
        chain
            .chain()
            .all(|cause| !cause.is::<super::StateFileInUse>()),
        "{chain:#}"
    );
    assert_eq!(
        chain
            .chain()
            .find_map(|cause| cause.downcast_ref::<std::io::Error>())
            .and_then(std::io::Error::raw_os_error),
        Some(5)
    );
    enter(&layout).unwrap();
    assert_eq!(read(&layout, RULES).as_deref(), Some("new"));
}
