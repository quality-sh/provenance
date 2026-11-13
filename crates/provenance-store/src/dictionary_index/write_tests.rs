//! The write of an index file: the new file replaces a stored index with one
//! rename, the directory has a sync after the rename and when an equal index
//! is stored, and a failed write keeps the stored file.
//!
//! No test stops the process or the electrical supply: each test shows the
//! order of the steps of one write.

use super::store_dictionary_index;
use crate::test_support::closed_directory::{close, Closed};
use crate::test_support::dictionary_import::import;
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

fn entry_names(directory: &Utf8Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(directory)
        .expect("list the directory")
        .map(|entry| {
            entry
                .expect("read an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
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

#[test]
#[verifies("rule_ste_dictionary_file_write_is_whole", examples)]
fn a_failed_write_of_the_new_index_keeps_the_stored_index() {
    let temp = tempfile::tempdir().expect("make a temporary directory");
    let (directory, path) = index_directory(&temp);
    std::fs::write(&path, STORED).expect("store an index");

    let closed = close(&directory, Closed::Writes);
    let result = store_dictionary_index(&import(), directory.as_std_path());
    drop(closed);

    assert!(result.is_err(), "unexpected result: {result:?}");
    assert_eq!(std::fs::read(&path).expect("read the index"), STORED);
    assert_eq!(entry_names(&directory), [file_name(&path)]);
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
