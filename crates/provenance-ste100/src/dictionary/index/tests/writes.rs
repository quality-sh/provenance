//! The write order of a stored index that differs from the import: the new
//! index replaces the stored index with one rename, and the directory has a
//! sync after the rename.
//!
//! Each test is an implementation aid with no Rule, because no Rule states
//! the durability of the write of a dictionary index.

use std::path::Path;

use super::super::{load_dictionary_index, store_dictionary_index, DictionaryIndexError};

use super::{fixture_import, scratch_directory};
use crate::dictionary::{digest, DictionaryImport};

/// An import with the same source as the fixture and other entries. Its
/// index file has the same path as the index of the fixture, with other
/// bytes.
fn other_import() -> DictionaryImport {
    let mut entries = fixture_import().entries;
    entries[0].approved_meaning_or_alternatives = "An other meaning".to_owned();
    DictionaryImport {
        identity: digest::identity(b"synthetic source bytes", &entries),
        entries,
    }
}

#[test]
fn a_failed_write_of_the_new_index_keeps_the_stored_index() {
    let directory = scratch_directory();
    let stored = fixture_import();
    let import = other_import();
    let path = store_dictionary_index(&stored, &directory).expect("store the first index");
    let staged = path.with_extension("partial");
    std::fs::create_dir(&staged).expect("make a directory at the staged path");

    let result = store_dictionary_index(&import, &directory);

    assert!(
        matches!(&result, Err(DictionaryIndexError::Io { path, .. }) if *path == staged),
        "unexpected result: {result:?}"
    );
    assert_eq!(
        load_dictionary_index(&directory, &stored.identity),
        Ok(stored),
        "a failed write must keep the stored index"
    );
    std::fs::remove_dir_all(&directory).expect("remove the scratch directory");
}

#[cfg(unix)]
#[test]
fn a_replace_of_a_stored_index_syncs_the_directory_after_the_rename() {
    let directory = scratch_directory();
    let path = store_dictionary_index(&fixture_import(), &directory).expect("store the index");
    let import = other_import();

    let Some(closed) = close_to_reads(&directory) else {
        std::fs::remove_dir_all(&directory).expect("remove the scratch directory");
        return;
    };
    let result = store_dictionary_index(&import, &directory);
    drop(closed);

    assert_eq!(
        result,
        Err(DictionaryIndexError::Io {
            path: directory.clone(),
            message: std::io::Error::from_raw_os_error(13).to_string(),
        })
    );
    assert_eq!(
        load_dictionary_index(&directory, &import.identity),
        Ok(import)
    );
    let mut names = entry_names(&directory);
    names.sort();
    let mut expected = vec![file_name(&path), file_name(&path.with_extension("lock"))];
    expected.sort();
    assert_eq!(names, expected, "no staged file must stay");
    std::fs::remove_dir_all(&directory).expect("remove the scratch directory");
}

#[cfg(unix)]
fn entry_names(directory: &Path) -> Vec<String> {
    std::fs::read_dir(directory)
        .expect("list the directory")
        .map(|entry| file_name(&entry.expect("read an entry").path()))
        .collect()
}

#[cfg(unix)]
fn file_name(path: &Path) -> String {
    path.file_name()
        .expect("the path has a file name")
        .to_string_lossy()
        .into_owned()
}

/// A directory that is closed to reads. The drop opens it again, so that a
/// failure or a panic does not keep it closed and the scratch directory can
/// be deleted.
#[cfg(unix)]
struct ClosedDirectory(std::path::PathBuf);

#[cfg(unix)]
impl Drop for ClosedDirectory {
    fn drop(&mut self) {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o755));
    }
}

/// Closes `directory` to reads: the process can write to it and search it.
/// When the process can open it all the same, as root can, the test cannot
/// observe the sync: the function opens the directory again, prints a line
/// and gives nothing.
#[cfg(unix)]
fn close_to_reads(directory: &Path) -> Option<ClosedDirectory> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o300))
        .expect("close the directory to reads");
    let closed = ClosedDirectory(directory.to_owned());
    if std::fs::File::open(directory).is_ok() {
        drop(closed);
        println!(
            "skipped: the process can open {} with no read permission",
            directory.display()
        );
        return None;
    }
    Some(closed)
}
