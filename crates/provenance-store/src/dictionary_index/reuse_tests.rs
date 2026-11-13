//! The index files in the machine index directory: an import stored one time
//! loads again with no PDF, and a second store of the same import does not
//! write the file again.

use super::{load_dictionary_index, store_dictionary_index};
use crate::test_support::dictionary_import::import;
use provenance_macros::verifies;

#[test]
#[verifies("rule_ste_dictionary_import_reuse", examples)]
fn a_stored_index_loads_again_without_the_source_document() {
    let directory = tempfile::tempdir().expect("make a temporary directory");
    let import = import();

    let path = store_dictionary_index(&import, directory.path()).expect("store the index");

    assert!(path.starts_with(directory.path()));
    assert_eq!(
        load_dictionary_index(directory.path(), &import.identity),
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
    let import = import();
    let first = store_dictionary_index(&import, directory.path()).expect("store the index");
    let stored_file = inode(&first);

    let second = store_dictionary_index(&import, directory.path()).expect("store it again");

    assert_eq!(second, first);
    assert_eq!(inode(&second), stored_file);
    assert_eq!(
        load_dictionary_index(directory.path(), &import.identity),
        Ok(import)
    );
}

#[cfg(unix)]
fn inode(path: &std::path::Path) -> u64 {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata(path)
        .expect("read the metadata of the index file")
        .ino()
}
