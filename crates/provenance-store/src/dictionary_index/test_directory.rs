//! The index directory of a store test.

use crate::test_support::dictionary_import::import;
use camino::{Utf8Path, Utf8PathBuf};
use provenance_ste100::dictionary_index_file_name;

/// The bytes of an index that an earlier store wrote.
pub const STORED: &[u8] = b"the bytes of a stored index";

/// A new index directory and the path of the index file of [`import`].
pub fn index_directory(temp: &tempfile::TempDir) -> (Utf8PathBuf, Utf8PathBuf) {
    let directory = Utf8Path::from_path(temp.path())
        .expect("the temporary directory is UTF-8")
        .join("index");
    std::fs::create_dir(&directory).expect("make the index directory");
    let path = directory.join(dictionary_index_file_name(&import().identity));
    (directory, path)
}

/// The file name of `path`.
pub fn file_name(path: &Utf8Path) -> String {
    path.file_name().expect("a file name").to_owned()
}
