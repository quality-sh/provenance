//! Writes while a reader has a file of the state open. A reader such as a
//! second repository open, an editor or a virus scanner can have the
//! publication marker or a shard open when a write replaces it.

use super::*;
use crate::state_store::RecordStamp::ResolveFromRepository as StampFromGit;
use crate::storage::local::LocalStateStorage;
use crate::storage::{StateKey, StateStorage};

/// This test has no Rule: no active Rule states that a write passes while a
/// reader has a file of the state open.
///
/// The probe opens the publication marker when the publication has moved the
/// live state to its backup, and keeps the handle until the publication
/// returns. The next marker writes replace the open marker.
#[test]
fn a_publication_passes_while_a_reader_has_the_marker_open() {
    let directory = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from_path_buf(directory.path().to_path_buf()).unwrap();
    let layout = ProvenanceLayout::new(root.clone());
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    let manifest = provenance_core::Manifest::default_with_scope(
        provenance_core::ScopeId::new("default").unwrap(),
        provenance_core::RepoPathPrefix::new("."),
    );
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    let held = std::rc::Rc::new(std::cell::RefCell::new(None));
    crate::test_probes::arm("state_after_backup_rename", {
        let held = held.clone();
        let marker = layout.publication_marker_path();
        move || {
            held.replace(Some(std::fs::File::open(&marker)?));
            Ok(())
        }
    });
    let mut published = manifest;
    published.disposition_actor_ids = vec!["actor_after".to_owned()];
    let result = with_repository_publication(&layout, |held| {
        with_staged_state(held, false, StampFromGit, |staged| {
            let manifest = staged.layout.manifest_path();
            staged.publish_shard(&manifest, serde_json::to_vec(&published)?)
        })
    });
    crate::test_probes::disarm("state_after_backup_rename");
    let handle = held.take().expect("the publication reached the swap");
    result.unwrap_or_else(|error| panic!("{error:#}"));
    drop(handle);
    let repository = crate::repository::Repository::open(root.as_std_path(), "default");
    assert_eq!(repository.unwrap().manifest().unwrap(), published);
}

/// This test has no Rule: no active Rule states that a write passes while a
/// reader has a file of the state open.
///
/// A reader keeps the shard open while a one-key set replaces it.
#[test]
fn a_one_key_write_passes_while_a_reader_has_the_shard_open() {
    let directory = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from_path_buf(directory.path().to_path_buf()).unwrap();
    let layout = ProvenanceLayout::new(root);
    let key = StateKey::new("scopes/default/sources/source.jsonl").unwrap();
    let shard = layout.state_dir().join(key.as_str());
    std::fs::create_dir_all(shard.parent().unwrap()).unwrap();
    std::fs::write(layout.manifest_path(), b"{}").unwrap();
    std::fs::write(&shard, b"{\"id\":\"before\"}\n").unwrap();
    let storage = LocalStateStorage::new(layout.clone());
    let after = b"{\"id\":\"after\"}\n".to_vec();
    let held = std::fs::File::open(&shard).unwrap();
    let writes = WriteSet::new(Atomicity::PerKey).put(key.clone(), after.clone());
    with_repository_publication(&layout, |_| Ok(storage.publish(writes)))
        .unwrap()
        .unwrap_or_else(|error| panic!("{error:#}"));
    let view = with_repository_publication(&layout, |_| Ok(storage.read_view()))
        .unwrap()
        .unwrap();
    drop(held);
    assert_eq!(view.entries().get(&key), Some(&after));
}
