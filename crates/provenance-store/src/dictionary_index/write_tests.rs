//! The write of an index file: the directory has a sync after the rename.
//!
//! No test stops the process or the electrical supply: each test shows the
//! order of the steps of one write.

use super::store_dictionary_index;
use super::test_directory::{file_name, index_directory, STORED};
use crate::test_support::closed_directory::{close, Closed};
use crate::test_support::dictionary_import::import;
use crate::test_support::directory::entry_names;
use provenance_macros::verifies;
use provenance_ste100::encode_dictionary_index;

#[test]
#[verifies("rule_ste_dictionary_file_write_is_durable", examples)]
fn a_replace_of_a_stored_index_syncs_the_directory_after_the_rename() {
    let temp = tempfile::tempdir().expect("make a temporary directory");
    let (directory, path) = index_directory(&temp);
    std::fs::write(&path, STORED).expect("store an index");

    let closed = close(&directory, Closed::Reads);
    let result = store_dictionary_index(&import(), directory.as_std_path());
    drop(closed);

    let error = result.expect_err("the directory sync must fail");
    assert_eq!(
        format!("{error:#}"),
        format!(
            "sync {directory}: {}",
            std::io::Error::from_raw_os_error(13)
        )
    );
    let written = encode_dictionary_index(&import()).expect("encode the index");
    assert_eq!(std::fs::read(&path).expect("read the index"), written);
    assert_eq!(entry_names(&directory), [file_name(&path)]);
}
