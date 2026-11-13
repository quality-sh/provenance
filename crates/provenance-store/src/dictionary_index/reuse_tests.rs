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

#[test]
#[verifies("rule_ste_dictionary_import_reuse", examples)]
fn storing_an_identical_index_again_is_idempotent() {
    let directory = tempfile::tempdir().expect("make a temporary directory");
    let import = import();
    let first = store_dictionary_index(&import, directory.path()).expect("store the index");
    let stored_at = modified(&first);

    let second = store_dictionary_index(&import, directory.path()).expect("store it again");

    assert_eq!(second, first);
    assert_eq!(modified(&second), stored_at);
    assert_eq!(
        load_dictionary_index(directory.path(), &import.identity),
        Ok(import)
    );
}

fn modified(path: &std::path::Path) -> std::time::SystemTime {
    std::fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .expect("read the time of the index file")
}
