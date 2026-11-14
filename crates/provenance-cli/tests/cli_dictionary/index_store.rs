//! The store of an index file in the machine index directory: an import
//! stored one time loads again with no PDF, a second store of the same import
//! does not write the file again, and a second store syncs the directory.

use super::support::imported_dictionary;
use provenance_macros::verifies;
use provenance_store::dictionary_index::{load_dictionary_index, store_dictionary_index};

#[test]
#[verifies("rule_ste_dictionary_import_reuse", examples)]
fn a_stored_index_loads_again_without_the_source_document() {
    let directory = tempfile::tempdir().expect("make a temporary directory");
    let import = imported_dictionary();

    let path = store_dictionary_index(import, directory.path()).expect("store the index");

    assert!(path.starts_with(directory.path()));
    assert_eq!(
        load_dictionary_index(directory.path(), &import.identity).as_ref(),
        Ok(import)
    );
}

/// A write of the index renames a new file over the path, so the file at the path
/// gets a new inode. The same inode shows that the second store did not write.
#[cfg(unix)]
#[test]
#[verifies("rule_ste_dictionary_import_reuse", examples)]
fn storing_an_identical_index_again_is_idempotent() {
    let directory = tempfile::tempdir().expect("make a temporary directory");
    let import = imported_dictionary();
    let first = store_dictionary_index(import, directory.path()).expect("store the index");
    let stored_file = inode(&first);

    let second = store_dictionary_index(import, directory.path()).expect("store it again");

    assert_eq!(second, first);
    assert_eq!(inode(&second), stored_file);
    assert_eq!(
        load_dictionary_index(directory.path(), &import.identity).as_ref(),
        Ok(import)
    );
}

/// The second store finds an equal index and does not write, but it syncs the
/// directory, because an earlier store can have stopped before its syncs. A
/// directory that the process cannot read makes that sync fail.
#[cfg(unix)]
#[test]
#[verifies("rule_ste_dictionary_file_write_is_durable", examples)]
fn a_store_of_an_equal_index_syncs_the_directory() {
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir().expect("make a temporary directory");
    let directory = temp.path().join("index");
    let import = imported_dictionary();
    let path = store_dictionary_index(import, &directory).expect("store the index");
    let stored = std::fs::read(&path).expect("read the index");

    let closed = std::fs::Permissions::from_mode(0o300);
    std::fs::set_permissions(&directory, closed).expect("close the directory to reads");
    let result = store_dictionary_index(import, &directory);
    let open = std::fs::Permissions::from_mode(0o700);
    std::fs::set_permissions(&directory, open).expect("open the directory again");

    let error = result.expect_err("the directory sync must fail");
    assert_eq!(
        format!("{error:#}"),
        format!(
            "sync {}: {}",
            directory.display(),
            std::io::Error::from_raw_os_error(13)
        )
    );
    assert_eq!(std::fs::read(&path).expect("read the index"), stored);
}

#[cfg(unix)]
fn inode(path: &std::path::Path) -> u64 {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata(path)
        .expect("read the metadata of the index file")
        .ino()
}
