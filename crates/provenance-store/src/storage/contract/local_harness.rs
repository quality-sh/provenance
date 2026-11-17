//! The contract harness of the local-disk adapter: a state directory in a
//! temporary directory, read and written in its publication section.

use super::{Harness, StateStorage};
use crate::layout::ProvenanceLayout;
use crate::storage::local::LocalStateStorage;
use camino::Utf8Path;

pub struct Local {
    /// The files of broken keys, held open until the harness drops, before
    /// the temporary directory is removed.
    #[cfg(windows)]
    held: Vec<std::fs::File>,
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

    /// A directory in the place of the file makes its rename fail, and the
    /// view leaves the key out. On Unix, a symbolic link in the directory
    /// also makes a copy of the state directory fail, so a set that copies
    /// the stored keys fails too.
    ///
    /// On Windows, the harness holds the file of a stored key open with a
    /// share mode that permits reads only. A write, a remove or a rename of
    /// that file then fails, and so does a rename of a directory above it,
    /// but a read succeeds and the view keeps the key.
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
            self.held.push(file);
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
}
