//! A write of an index file that stops before its rename keeps the stored
//! file, and no temporary file stays.
//!
//! No test stops the process: the labelled probe stops the write with an
//! error between the sync of the temporary file and the rename.

use super::store_dictionary_index;
use super::test_directory::{file_name, index_directory, STORED};
use crate::test_support::dictionary_import::import;
use crate::test_support::directory::entry_names;
use provenance_macros::verifies;

/// The write stops with an error at the point between the sync of the
/// temporary file and the rename. A stop of the process at that point also
/// leaves the temporary file in the directory.
#[test]
#[verifies("rule_ste_dictionary_file_write_is_whole", examples)]
fn a_write_stopped_before_its_rename_keeps_the_stored_index() {
    let temp = tempfile::tempdir().expect("make a temporary directory");
    let (directory, path) = index_directory(&temp);
    std::fs::write(&path, STORED).expect("store an index");

    crate::test_probes::crash_at("synced_file_before_rename");
    let result = store_dictionary_index(&import(), directory.as_std_path());
    crate::test_probes::disarm("synced_file_before_rename");

    let error = result.expect_err("the write must stop before its rename");
    assert_eq!(
        format!("{error:#}"),
        "injected crash at synced_file_before_rename"
    );
    assert_eq!(std::fs::read(&path).expect("read the index"), STORED);
    assert_eq!(entry_names(&directory), [file_name(&path)]);
}
