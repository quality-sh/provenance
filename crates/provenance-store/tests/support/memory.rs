//! A repository over the memory adapter, rooted at an empty directory.
//!
//! The state starts with the manifest, which declares the default scope and
//! the scope `imported`, one active Requirement and one active Rule. A test
//! can add more state entries. The adapter gives each write set to a
//! persistence that keeps it.

#![allow(dead_code)]

use provenance_core::{Manifest, RepoPathPrefix, Scope, ScopeId, StableId};
use provenance_store::repository::{Repository, RepositoryIdentity};
use provenance_store::settings::Settings;
use provenance_store::storage::memory::{MemorySectionLock, MemoryStateStorage};
use provenance_store::storage::{
    PublishFailure, StateKey, StatePersistence, StateStorage, StateView, WriteSet,
};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

/// Keeps each write set that the adapter gives it.
#[derive(Debug, Default)]
pub struct Recorded(pub Mutex<Vec<WriteSet>>);

impl StatePersistence for Recorded {
    fn persist(&self, writes: &WriteSet) -> Result<(), PublishFailure> {
        self.0.lock().unwrap().push(writes.clone());
        Ok(())
    }
}

pub fn scope() -> ScopeId {
    ScopeId::new("default").unwrap()
}

pub fn imported() -> ScopeId {
    ScopeId::new("imported").unwrap()
}

pub fn id(value: &str) -> StableId {
    StableId::new(value).unwrap()
}

pub fn key(value: &str) -> StateKey {
    StateKey::new(value).unwrap()
}

/// The JSONL bytes of `rows`.
pub fn shard(rows: &[serde_json::Value]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for row in rows {
        bytes.extend(serde_json::to_vec(row).unwrap());
        bytes.push(b'\n');
    }
    bytes
}

/// The manifest, one active Requirement and one active Rule.
pub fn initial_state() -> StateView {
    StateView::new(initial_entries())
}

fn initial_entries() -> BTreeMap<StateKey, Vec<u8>> {
    let mut manifest = Manifest::default_with_scope(scope(), RepoPathPrefix::new("."));
    manifest.scopes.push(Scope {
        id: imported(),
        path_prefix: RepoPathPrefix::new("imported"),
    });
    let requirement = serde_json::json!({
        "scope_id": "default",
        "id": "req_expiry",
        "statement": "Share links expire",
        "status": "active",
    });
    let rule = serde_json::json!({
        "scope_id": "default",
        "id": "rule_expiry",
        "statement": "Share links expire",
        "status": "active",
        "severity": "medium",
        "requirement_ids": ["req_expiry"],
    });
    BTreeMap::from([
        (key("manifest.json"), serde_json::to_vec(&manifest).unwrap()),
        (
            key("scopes/default/requirements/req.jsonl"),
            shard(&[requirement]),
        ),
        (key("scopes/default/rules/rule.jsonl"), shard(&[rule])),
    ])
}

pub struct MemoryRepository {
    pub root: tempfile::TempDir,
    pub storage: Arc<MemoryStateStorage>,
    pub recorded: Arc<Recorded>,
    pub repository: Repository,
    initial: StateView,
}

impl MemoryRepository {
    /// The working tree has no entry.
    pub fn assert_empty(&self) {
        let entries: Vec<_> = std::fs::read_dir(self.root.path())
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        assert!(
            entries.is_empty(),
            "the repository wrote files: {entries:?}"
        );
    }

    /// The state that the recorded write sets make from the initial state.
    pub fn replayed(&self) -> StateView {
        let mut entries = self.initial.clone().into_entries();
        for writes in self.recorded.0.lock().unwrap().iter() {
            writes.apply_to(&mut entries);
        }
        MemoryStateStorage::new(entries).read_view().unwrap()
    }
}

pub fn memory_repository() -> MemoryRepository {
    memory_repository_with([])
}

/// A memory repository whose state also holds `extra`.
pub fn memory_repository_with(
    extra: impl IntoIterator<Item = (StateKey, Vec<u8>)>,
) -> MemoryRepository {
    let mut entries = initial_entries();
    entries.extend(extra);
    let initial = StateView::new(entries);
    let root = tempfile::tempdir().unwrap();
    let recorded = Arc::new(Recorded::default());
    let storage = Arc::new(MemoryStateStorage::loaded(
        initial.clone(),
        recorded.clone(),
    ));
    let working_tree =
        camino::Utf8PathBuf::from_path_buf(dunce::canonicalize(root.path()).unwrap()).unwrap();
    let identity = RepositoryIdentity::new(working_tree, scope(), Settings::default());
    let repository = Repository::with_storage(
        identity,
        storage.clone(),
        Arc::new(MemorySectionLock::default()),
    )
    .unwrap();
    MemoryRepository {
        root,
        storage,
        recorded,
        repository,
        initial,
    }
}
