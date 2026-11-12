//! The sync of the directory in a per-key write, seen through a directory
//! that the process can write to but cannot open to read.

use super::LocalStateStorage;
use crate::layout::ProvenanceLayout;
use crate::publication::with_repository_publication;
use crate::storage::{Atomicity, PublishFailure, StateKey, StateStorage, WriteSet};
use camino::Utf8Path;
use std::os::unix::fs::PermissionsExt;

const KEY: &str = "scopes/a/rules/rule.jsonl";

/// An implementation aid with no Rule, because no Rule states the
/// durability of a per-key write.
#[test]
fn a_per_key_write_syncs_the_directory_after_the_rename() -> anyhow::Result<()> {
    let temp = tempfile::tempdir().unwrap();
    let layout = ProvenanceLayout::new(Utf8Path::from_path(temp.path()).unwrap());
    let path = layout.state_dir().join(KEY);
    let directory = path.parent().unwrap().to_owned();
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(&path, b"old").unwrap();
    let storage = LocalStateStorage::new(layout.clone());
    let key = StateKey::new(KEY).unwrap();
    let writes = WriteSet::new(Atomicity::PerKey).put(key, b"new".to_vec());

    // Write and search permission let the rename occur; with no read
    // permission, the open of the directory for its sync fails.
    set_mode(&directory, 0o300);
    let result = with_repository_publication(&layout, |_| Ok(storage.publish(writes)))?;
    set_mode(&directory, 0o755);

    let Err(PublishFailure::Started(error)) = result else {
        panic!("the directory sync did not fail: {result:?}");
    };
    assert_eq!(
        error.to_string(),
        std::io::Error::from_raw_os_error(13).to_string()
    );
    assert_eq!(std::fs::read(&path).unwrap(), b"new");
    assert_eq!(std::fs::read_dir(&directory)?.count(), 1);
    Ok(())
}

fn set_mode(path: &Utf8Path, mode: u32) {
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
}
