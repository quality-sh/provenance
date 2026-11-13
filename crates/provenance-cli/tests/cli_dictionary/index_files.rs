//! The index files in the machine index directory: an import stored one time
//! loads again with no PDF, and a second store of the same import changes
//! nothing.

use crate::support::imported_dictionary;
use provenance_macros::verifies;
use provenance_ste100::DictionaryIndexError;
use provenance_store::dictionary_index::{load_dictionary_index, store_dictionary_index};

#[test]
#[verifies("rule_ste_dictionary_import_reuse", examples)]
fn a_stored_index_loads_again_without_the_source_document() {
    let directory = tempfile::tempdir().unwrap();
    let import = imported_dictionary();

    let path = store_dictionary_index(import, directory.path()).unwrap();

    assert!(path.starts_with(directory.path()));
    assert_eq!(
        load_dictionary_index(directory.path(), &import.identity).as_ref(),
        Ok(import)
    );
}

#[test]
#[verifies("rule_ste_dictionary_import_reuse", examples)]
fn storing_an_identical_index_again_is_idempotent() {
    let directory = tempfile::tempdir().unwrap();
    let import = imported_dictionary();
    let first = store_dictionary_index(import, directory.path()).unwrap();
    let modified = std::fs::metadata(&first).unwrap().modified().unwrap();

    let second = store_dictionary_index(import, directory.path()).unwrap();

    assert_eq!(second, first);
    assert_eq!(
        std::fs::metadata(&second).unwrap().modified().unwrap(),
        modified
    );
    assert_eq!(
        load_dictionary_index(directory.path(), &import.identity).as_ref(),
        Ok(import)
    );
}

#[test]
fn a_missing_index_reports_not_found() {
    let directory = tempfile::tempdir().unwrap();
    let import = imported_dictionary();

    let error = load_dictionary_index(directory.path(), &import.identity);

    assert!(
        matches!(error, Err(DictionaryIndexError::NotFound { .. })),
        "unexpected result: {error:?}"
    );
}
