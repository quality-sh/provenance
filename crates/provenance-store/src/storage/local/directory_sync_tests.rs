//! The sync of a directory in a synced write and in a synced remove, seen
//! through a directory that the process can write to and search but cannot
//! open to read: the change lands, and the open of the directory for its
//! sync fails.
//!
//! Each test is an implementation aid with no Rule, because no Rule states
//! the durability of a write to the local disk.

use super::LocalStateStorage;
use crate::layout::ProvenanceLayout;
use crate::publication::{with_repository_publication, write_publication_marker, PublicationPhase};
use crate::storage::{Atomicity, PublishFailure, StateKey, StateStorage, WriteSet};
use crate::test_support::closed_directory::{close, Closed};
use crate::test_support::directory::utf8;
use camino::{Utf8Path, Utf8PathBuf};

const KEY: &str = "scopes/a/rules/rule.jsonl";

#[test]
fn a_per_key_put_syncs_the_directory_after_the_rename() -> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let layout = layout_in(&temp);
    let path = layout.state_dir().join(KEY);
    let directory = parent(&path)?;
    std::fs::create_dir_all(&directory)?;
    std::fs::write(&path, b"old")?;
    let key = StateKey::new(KEY)?;
    let writes = WriteSet::new(Atomicity::PerKey).put(key, b"new".to_vec());

    let closed = close(&directory, Closed::Reads);
    let result = publish(&layout, writes);
    drop(closed);

    assert_eq!(started_text(result?), sync_failed(&directory));
    assert_eq!(std::fs::read(&path)?, b"new");
    assert_eq!(std::fs::read_dir(&directory)?.count(), 1);
    Ok(())
}

#[test]
fn a_per_key_delete_syncs_the_directory_after_the_remove() -> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let layout = layout_in(&temp);
    let path = layout.state_dir().join(KEY);
    let directory = parent(&path)?;
    std::fs::create_dir_all(&directory)?;
    std::fs::write(&path, b"old")?;
    let key = StateKey::new(KEY)?;
    let writes = WriteSet::new(Atomicity::PerKey).delete(key);

    let closed = close(&directory, Closed::Reads);
    let result = publish(&layout, writes);
    drop(closed);

    assert_eq!(started_text(result?), sync_failed(&directory));
    assert!(!path.try_exists()?);
    Ok(())
}

#[test]
fn a_per_key_put_syncs_the_parent_of_a_new_directory() -> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let layout = layout_in(&temp);
    let path = layout.state_dir().join(KEY);
    let new_directory = parent(&path)?;
    let scope = parent(&new_directory)?;
    std::fs::create_dir_all(&scope)?;
    let key = StateKey::new(KEY)?;
    let writes = WriteSet::new(Atomicity::PerKey).put(key, b"new".to_vec());

    let closed = close(&scope, Closed::Reads);
    let result = publish(&layout, writes);
    drop(closed);

    assert_eq!(started_text(result?), sync_failed(&scope));
    assert!(new_directory.is_dir());
    assert_eq!(std::fs::read(&path)?, b"new");
    assert_eq!(std::fs::read_dir(&new_directory)?.count(), 1);
    Ok(())
}

/// A whole set installs each put with a rename onto the live key and then a
/// sync of its directory. The failed sync comes after the marker, so the
/// write succeeds, and the marker stays for the next section entry. The
/// directory closes after the write staged its files, because the write
/// reads each directory of the state before its marker.
#[test]
fn a_whole_set_install_syncs_the_directory_after_each_rename() -> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let layout = layout_in(&temp);
    let path = layout.state_dir().join(KEY);
    let directory = parent(&path)?;
    std::fs::create_dir_all(&directory)?;
    std::fs::write(&path, b"old")?;
    let other = "scopes/a/requirements/req.jsonl";
    let writes = WriteSet::new(Atomicity::WholeSet)
        .put(StateKey::new(KEY)?, b"new".to_vec())
        .put(StateKey::new(other)?, b"other".to_vec());
    let closed = std::rc::Rc::new(std::cell::RefCell::new(None));
    let held = std::rc::Rc::clone(&closed);
    let staged = directory.clone();
    crate::test_probes::arm("state_prepared", move || {
        held.borrow_mut().replace(close(&staged, Closed::Reads));
        Ok(())
    });

    let result = publish(&layout, writes);
    crate::test_probes::disarm("state_prepared");
    drop(closed.borrow_mut().take());

    result?.map_err(|error| anyhow::anyhow!("{error}"))?;
    assert_eq!(std::fs::read(&path)?, b"new");
    assert_eq!(std::fs::read_dir(&directory)?.count(), 1);
    assert!(layout.publication_marker_path().is_file());
    Ok(())
}

#[test]
fn a_jsonl_file_write_syncs_the_directory_after_the_rename() -> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let root = utf8(temp.path());
    let side = root.join("side.jsonl");
    std::fs::write(&side, "{\"id\":\"new\"}\n")?;
    let directory = root.join("target");
    let path = directory.join("records.jsonl");
    std::fs::create_dir(&directory)?;
    std::fs::write(&path, "{\"id\":\"old\"}\n")?;
    let rows = crate::merge::read_jsonl_rows(&side, crate::merge::MergeSide::Ours)?;
    let records = [serde_json::json!({ "id": "new" })];

    let closed = close(&directory, Closed::Reads);
    let result = crate::jsonl::write_preserved_jsonl_atomic(&path, &rows, &[], &records);
    drop(closed);

    let Err(error) = result else {
        panic!("the directory sync did not fail");
    };
    assert_eq!(format!("{error:#}"), sync_failed(&directory));
    assert_eq!(std::fs::read_to_string(&path)?, "{\"id\":\"new\"}\n");
    assert_eq!(std::fs::read_dir(&directory)?.count(), 1);
    Ok(())
}

#[test]
fn a_publication_marker_write_syncs_the_cache_directory() -> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let layout = layout_in(&temp);
    let cache = layout.cache_dir();
    let transaction = layout.import_transactions_dir().join("transaction");
    std::fs::create_dir_all(&transaction)?;

    let closed = close(&cache, Closed::Reads);
    let result = write_publication_marker(&layout, &transaction, PublicationPhase::Installing);
    drop(closed);

    let Err(error) = result else {
        panic!("the directory sync did not fail");
    };
    assert_eq!(format!("{error:#}"), sync_failed(&cache));
    let marker: serde_json::Value =
        serde_json::from_slice(&std::fs::read(layout.publication_marker_path())?)?;
    assert_eq!(marker["phase"], "installing");
    Ok(())
}

fn layout_in(temp: &tempfile::TempDir) -> ProvenanceLayout {
    ProvenanceLayout::new(utf8(temp.path()))
}

fn parent(path: &Utf8Path) -> anyhow::Result<Utf8PathBuf> {
    path.parent()
        .map(Utf8Path::to_owned)
        .ok_or_else(|| anyhow::anyhow!("{path} has no parent"))
}

fn publish(
    layout: &ProvenanceLayout,
    writes: WriteSet,
) -> anyhow::Result<Result<(), PublishFailure>> {
    let storage = LocalStateStorage::new(layout.clone());
    with_repository_publication(layout, |_| Ok(storage.publish(writes)))
}

/// The complete text of a publish that failed after its key landed.
fn started_text(result: Result<(), PublishFailure>) -> String {
    let Err(PublishFailure::Started(error)) = result else {
        panic!("the directory sync did not fail: {result:?}");
    };
    format!("{error:#}")
}

/// The complete text of a failed sync of `directory`.
fn sync_failed(directory: &Utf8Path) -> String {
    format!(
        "sync {directory}: {}",
        std::io::Error::from_raw_os_error(13)
    )
}
