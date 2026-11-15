//! The bytes of the stored files of a repository, so a test can show that a
//! refused command changes no file.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The bytes of each file below `.provenance/state` of the repository at
/// `repo`, by path.
pub fn state_files(repo: impl AsRef<Path>) -> BTreeMap<PathBuf, Vec<u8>> {
    files_below(repo.as_ref().join(".provenance/state"))
}

/// The bytes of each file below `directory`, by path.
pub fn files_below(directory: impl AsRef<Path>) -> BTreeMap<PathBuf, Vec<u8>> {
    fn read(directory: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                read(&path, files);
            } else {
                files.insert(path.clone(), std::fs::read(path).unwrap());
            }
        }
    }
    let mut files = BTreeMap::new();
    read(directory.as_ref(), &mut files);
    files
}
