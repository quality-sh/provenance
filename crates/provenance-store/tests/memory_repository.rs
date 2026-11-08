//! A repository over the memory adapter writes records with no file access.
//!
//! The repository is rooted at an empty directory. Each write set reaches
//! the persistence of the adapter, and the directory stays empty. The state
//! starts with the manifest, one Requirement and one Rule; live writes make
//! and change a verification binding of the Rule.

use provenance_core::{Manifest, RepoPathPrefix, ScopeId, StableId, VerificationMethod};
use provenance_macros::verifies;
use provenance_store::repository::{Repository, RepositoryIdentity};
use provenance_store::settings::Settings;
use provenance_store::state_store::MaterializeVerificationBindingInput;
use provenance_store::storage::memory::{MemorySectionLock, MemoryStateStorage};
use provenance_store::storage::{
    PublishFailure, StateKey, StatePersistence, StateStorage, StateView, WriteSet,
};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

/// Keeps each write set that the adapter gives it.
#[derive(Debug, Default)]
struct Recorded(Mutex<Vec<WriteSet>>);

impl StatePersistence for Recorded {
    fn persist(&self, writes: &WriteSet) -> Result<(), PublishFailure> {
        self.0.lock().unwrap().push(writes.clone());
        Ok(())
    }
}

fn scope() -> ScopeId {
    ScopeId::new("default").unwrap()
}

fn id(value: &str) -> StableId {
    StableId::new(value).unwrap()
}

fn shard(rows: &[serde_json::Value]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for row in rows {
        bytes.extend(serde_json::to_vec(row).unwrap());
        bytes.push(b'\n');
    }
    bytes
}

/// A state with the manifest, which declares the default scope, one active
/// Requirement and one active Rule under it.
fn initial_state() -> StateView {
    let manifest = Manifest::default_with_scope(scope(), RepoPathPrefix::new("."));
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
    let key = |key: &str| StateKey::new(key).unwrap();
    StateView::new(BTreeMap::from([
        (key("manifest.json"), serde_json::to_vec(&manifest).unwrap()),
        (
            key("scopes/default/requirements/req.jsonl"),
            shard(&[requirement]),
        ),
        (key("scopes/default/rules/rule.jsonl"), shard(&[rule])),
    ]))
}

struct MemoryRepository {
    root: tempfile::TempDir,
    storage: Arc<MemoryStateStorage>,
    recorded: Arc<Recorded>,
    repository: Repository,
}

fn memory_repository() -> MemoryRepository {
    let root = tempfile::tempdir().unwrap();
    let recorded = Arc::new(Recorded::default());
    let storage = Arc::new(MemoryStateStorage::loaded(
        initial_state(),
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
    }
}

/// Makes or changes the binding of the Rule with the key `share-link-expiry`.
fn bind(repository: &Repository, file: &str) {
    repository
        .store()
        .materialize_verification_binding(MaterializeVerificationBindingInput {
            scope_id: scope(),
            rule_id: id("rule_expiry"),
            key: "share-link-expiry".into(),
            method: VerificationMethod::Examples,
            declared_by: "test://typescript".into(),
            file: file.into(),
            symbol: None,
        })
        .unwrap();
}

/// The working tree has no entry.
fn assert_empty(root: &tempfile::TempDir) {
    let entries: Vec<_> = std::fs::read_dir(root.path())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert!(
        entries.is_empty(),
        "the repository wrote files: {entries:?}"
    );
}

/// The state that the recorded write sets make from the initial state.
fn replayed(recorded: &Recorded) -> StateView {
    let mut entries = initial_state().into_entries();
    for writes in recorded.0.lock().unwrap().iter() {
        writes.apply_to(&mut entries);
    }
    MemoryStateStorage::new(entries).read_view().unwrap()
}

#[test]
#[verifies("rule_engine_runs_on_any_storage", examples)]
fn a_repository_over_memory_writes_live_records_with_no_file() {
    let memory = memory_repository();
    bind(&memory.repository, "tests/share-links.test.ts");
    bind(&memory.repository, "tests/expiry.test.ts");

    assert_empty(&memory.root);
    let view = memory.repository.view().unwrap();
    assert_eq!(view.list_requirements(&scope()).unwrap().len(), 1);
    assert_eq!(view.list_rules(&scope()).unwrap().len(), 1);
    let bindings = view.list_verification_bindings(&scope()).unwrap();
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].file.as_str(), "tests/expiry.test.ts");
    assert_eq!(memory.recorded.0.lock().unwrap().len(), 2);
    assert_eq!(
        replayed(&memory.recorded).revision(),
        memory.storage.read_view().unwrap().revision()
    );
}
