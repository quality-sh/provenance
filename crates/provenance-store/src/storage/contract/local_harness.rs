//! The contract harness of the local-disk adapter: a state directory in a
//! temporary directory, read and written in its publication section.

use super::{Harness, StateStorage};
use crate::layout::ProvenanceLayout;
use crate::storage::local::LocalStateStorage;
use camino::Utf8Path;

pub struct Local {
    /// The files of broken keys, held open until the harness drops or
    /// repairs the key, before the temporary directory is removed.
    #[cfg(windows)]
    held: Vec<(String, std::fs::File)>,
    _temp: tempfile::TempDir,
    pub layout: ProvenanceLayout,
    storage: LocalStateStorage,
}

pub fn local() -> Local {
    let temp = tempfile::tempdir().unwrap();
    let layout = ProvenanceLayout::new(Utf8Path::from_path(temp.path()).unwrap());
    Local {
        #[cfg(windows)]
        held: Vec::new(),
        storage: LocalStateStorage::new(layout.clone()),
        layout,
        _temp: temp,
    }
}

impl Harness for Local {
    fn seed(&mut self, entries: &[(&str, &[u8])]) {
        for (key, bytes) in entries {
            let path = self.layout.state_dir().join(key);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, bytes).unwrap();
        }
    }

    fn with_storage<R>(&self, call: impl FnOnce(&dyn StateStorage) -> R) -> R {
        crate::publication::with_repository_publication(&self.layout, |_| Ok(call(&self.storage)))
            .unwrap()
    }

    /// On Linux and macOS, the harness puts a directory with a file and a
    /// link in the place of the key. A write cannot replace it, so a set
    /// that changes the key is refused before its marker, and the view
    /// refuses the link.
    ///
    /// On Windows, the harness holds the file of a stored key open with a
    /// share mode that permits reads only. A replace or a removal of that
    /// file then fails after the marker of a whole set, but a read succeeds
    /// and the view keeps the key.
    fn break_key(&mut self, key: &str) {
        let path = self.layout.state_dir().join(key);
        #[cfg(windows)]
        if path.is_file() {
            use std::os::windows::fs::OpenOptionsExt;
            const FILE_SHARE_READ: u32 = 0x1;
            let file = std::fs::OpenOptions::new()
                .read(true)
                .share_mode(FILE_SHARE_READ)
                .open(&path)
                .unwrap();
            self.held.push((key.to_owned(), file));
            return;
        }
        if path.is_file() {
            std::fs::remove_file(&path).unwrap();
        }
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("keep"), b"").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(path.join("keep"), path.join("link")).unwrap();
    }

    /// Closes the held file of `key`, or removes the directory in its place.
    fn repair_key(&mut self, key: &str) {
        #[cfg(windows)]
        self.held.retain(|(held, _)| held != key);
        let path = self.layout.state_dir().join(key);
        if path.is_dir() {
            std::fs::remove_dir_all(&path).unwrap();
        }
    }
}
