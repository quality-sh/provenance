//! The bytes of the state files of a repository, so a test can show that a
//! refused command changes no file.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The bytes of each file below `.provenance/state` of the repository at
/// `repo`, by path.
pub fn state_files(repo: impl AsRef<Path>) -> BTreeMap<PathBuf, Vec<u8>> {
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
    read(&repo.as_ref().join(".provenance/state"), &mut files);
    files
}
