//! The path and the entries of a test directory.

use camino::{Utf8Path, Utf8PathBuf};

/// `path` as a UTF-8 path. A path that is not UTF-8 stops the test.
pub fn utf8(path: &std::path::Path) -> Utf8PathBuf {
    Utf8PathBuf::from_path_buf(path.to_path_buf())
        .unwrap_or_else(|path| panic!("{} is not UTF-8", path.display()))
}

/// The names of the entries of `directory`, sorted.
pub fn entry_names(directory: &Utf8Path) -> Vec<String> {
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
