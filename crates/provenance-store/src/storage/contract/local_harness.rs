//! The contract harness of the local-disk adapter: a state directory in a
//! temporary directory, read and written in its publication section.

use super::{Harness, StateStorage};
use crate::layout::ProvenanceLayout;
use crate::storage::local::LocalStateStorage;
use camino::Utf8Path;

pub struct Local {
    _temp: tempfile::TempDir,
    pub layout: ProvenanceLayout,
    storage: LocalStateStorage,
}

pub fn local() -> Local {
    let temp = tempfile::tempdir().unwrap();
    let layout = ProvenanceLayout::new(Utf8Path::from_path(temp.path()).unwrap());
    Local {
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

    /// A directory in the place of the file makes its rename fail.
    fn break_key(&mut self, key: &str) {
        let path = self.layout.state_dir().join(key);
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("keep"), b"").unwrap();
    }
}
