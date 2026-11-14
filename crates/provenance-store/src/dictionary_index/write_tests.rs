//! The write of an index file: the new file replaces a stored index with one
//! rename, the directory has a sync after the rename and when an equal index
//! is stored, and a write that stops before its rename keeps the stored file.
//!
//! No test stops the process or the electrical supply: each test shows the
//! order of the steps of one write.

use super::store_dictionary_index;
use crate::test_support::closed_directory::{close, Closed};
use crate::test_support::dictionary_import::import;
use crate::test_support::directory::entry_names;
use camino::{Utf8Path, Utf8PathBuf};
use provenance_macros::verifies;
use provenance_ste100::{dictionary_index_file_name, encode_dictionary_index};

const STORED: &[u8] = b"the bytes of a stored index";

/// A new index directory and the path of the index file of [`import`].
fn index_directory(temp: &tempfile::TempDir) -> (Utf8PathBuf, Utf8PathBuf) {
    let directory = Utf8Path::from_path(temp.path())
        .expect("the temporary directory is UTF-8")
        .join("index");
    std::fs::create_dir(&directory).expect("make the index directory");
    let path = directory.join(dictionary_index_file_name(&import().identity));
    (directory, path)
}

fn file_name(path: &Utf8Path) -> String {
    path.file_name().expect("a file name").to_owned()
}

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
}

#[test]
#[verifies("rule_ste_dictionary_file_write_is_durable", examples)]
fn a_store_of_an_equal_index_syncs_the_directory() {
    let temp = tempfile::tempdir().expect("make a temporary directory");
    let (directory, path) = index_directory(&temp);
    store_dictionary_index(&import(), directory.as_std_path()).expect("store the index");

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
