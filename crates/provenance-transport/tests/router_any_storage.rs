//! The contract router over a repository whose state is in memory gives the
//! answers of the router over the same state in a folder.
//!
//! The repository in memory has a working tree that does not exist, and it
//! runs its read operations through the in-process port. Its read database
//! is in a directory of its own. The run has an empty home directory and an
//! empty working directory, and they stay empty.
//!
//! No Rule: the plan proposes `rule_contract_router_serves_any_storage` for
//! the owner's review. Until it is approved, this test holds the behaviour.
#![cfg(feature = "test-fixture")]

// Each test file uses a part of the shared support.
#[allow(dead_code)]
mod support {
    pub mod catch_up_steps;
    pub mod records;
    pub mod resource_http;
    pub mod site_operations;
    pub mod transcript;
}

use camino::Utf8PathBuf;
use provenance_core::ScopeId;
use provenance_store::layout::ProvenanceLayout;
use provenance_store::operations::read_operations::{
    InProcessReadOperations, ReadAnswer, ReadOperations, READ_OPERATIONS,
};
use provenance_store::read_database::file::FileReadDatabase;
use provenance_store::repository::{Repository as Opened, RepositoryIdentity};
use provenance_store::settings::Settings;
use provenance_store::storage::memory::{MemorySectionLock, MemoryStateStorage};
use provenance_store::storage::StateKey;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::{Arc, Mutex};
use support::records::{allow_reviewer, Repository};
use support::site_operations::{run, Http, READS};

/// The in-process port, which keeps the name of each operation it runs.
#[derive(Debug)]
struct Counted {
    reads: InProcessReadOperations,
    runs: Arc<Mutex<Vec<&'static str>>>,
}

impl ReadOperations for Counted {
    fn run(&self, operation: &'static str, request: Value) -> ReadAnswer {
        self.runs.lock().unwrap().push(operation);
        self.reads.run(operation, request)
    }
}

/// The state files of `repo`, keyed as the storage keys them.
fn state(repo: &Repository) -> BTreeMap<StateKey, Vec<u8>> {
    let state = repo.layout.state_dir().into_std_path_buf();
    let mut entries = BTreeMap::new();
    let mut pending = vec![state.clone()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            let key = path.strip_prefix(&state).unwrap().to_str().unwrap();
            let key = StateKey::new(key.replace('\\', "/")).unwrap();
            entries.insert(key, std::fs::read(&path).unwrap());
        }
    }
    entries
}

/// The entries of `directory`.
fn entries(directory: &Path) -> Vec<std::path::PathBuf> {
    std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect()
}

#[tokio::test]
async fn the_router_over_memory_storage_answers_as_over_a_folder() {
    let folder = Repository::new("The shared graph is readable.");
    folder.all_kinds();
    allow_reviewer(&folder);
    let settings = Settings::load(&folder.layout).unwrap();
    let initial = state(&folder);

    let home = tempfile::tempdir().unwrap();
    let working = tempfile::tempdir().unwrap();
    let missing = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from_path_buf(missing.path().join("project")).unwrap();
    let read_root = tempfile::tempdir().unwrap();
    let read_layout = ProvenanceLayout::new(read_root.path().to_str().unwrap());
    std::fs::create_dir_all(read_layout.cache_dir()).unwrap();

    let expected = run(
        &Http::over(folder.open()),
        folder.dir.path().to_str().unwrap(),
    )
    .await
    .named();

    std::env::set_var("HOME", home.path());
    std::env::set_current_dir(working.path()).unwrap();
    let identity =
        RepositoryIdentity::new(root.clone(), ScopeId::new("default").unwrap(), settings);
    let memory = Opened::with_storage(
        identity,
        Arc::new(MemoryStateStorage::new(initial)),
        Arc::new(MemorySectionLock::default()),
        Some(Arc::new(FileReadDatabase::new(read_layout))),
    )
    .unwrap();
    let runs = Arc::new(Mutex::new(Vec::new()));
    let memory = memory.with_read_operations(Arc::new(Counted {
        reads: InProcessReadOperations::new(memory.clone()),
        runs: runs.clone(),
    }));
    let actual = run(&Http::over(memory), root.as_str()).await.named();

    for (index, (left, right)) in actual.iter().zip(&expected).enumerate() {
        assert_eq!(left, right, "call {index}");
    }
    assert_eq!(actual.len(), expected.len());
    let runs = runs.lock().unwrap().clone();
    assert_eq!(runs.len(), READS, "{runs:?}");
    assert_eq!(
        runs.iter().copied().collect::<BTreeSet<_>>(),
        READ_OPERATIONS.iter().copied().collect::<BTreeSet<_>>()
    );
    assert!(!root.exists(), "the router made the working tree");
    assert_eq!(entries(missing.path()), Vec::<std::path::PathBuf>::new());
    assert_eq!(entries(home.path()), Vec::<std::path::PathBuf>::new());
    assert_eq!(entries(working.path()), Vec::<std::path::PathBuf>::new());
}
