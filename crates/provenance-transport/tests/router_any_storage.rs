//! The contract router over a repository whose state is in memory gives the
//! answers of the router over the same state in a folder.
//!
//! The repository in memory has a working tree that does not exist and a
//! read database in memory, and it runs its read operations through the
//! in-process port. The home directory, the working directory and the
//! temporary directory of the run are new empty directories. They stay
//! empty, and so does the directory that would hold the working tree.
//!
//! Each answer must equal the answer over the folder in its status, its
//! headers and its body bytes, and in its projection stamp apart from the
//! read database's instance id (see `support/memory_host.rs` for each part
//! that may differ, and why). The answers over memory must also equal the
//! checked transcript of the characterization.
//!
//! No Rule: the plan proposes `rule_contract_router_serves_any_storage` for
//! the owner's review. Until it is approved, this test holds the behaviour.
#![cfg(feature = "test-fixture")]

mod support {
    pub mod memory_host;
    pub mod names;
    pub mod records;
    pub mod router_answer;
    pub mod site_operations;
    pub mod views;
}

use camino::Utf8PathBuf;
use provenance_store::operations::read_operations::{InProcessReadOperations, READ_OPERATIONS};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use support::memory_host::{compared, memory_repository, Counted};
use support::names::Names;
use support::records::{allow_reviewer, Repository};
use support::router_answer::Http;
use support::site_operations::{assert_pinned, run};

/// The number of calls of `run` to read operations: the 25 reads, the read
/// of the new Requirement, and each record again at the end.
const READS: usize = 25 + 1 + 6;

/// The entries of `directory`.
fn entries(directory: &Path) -> Vec<PathBuf> {
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
    let home = tempfile::tempdir().unwrap();
    let working = tempfile::tempdir().unwrap();
    let temporary = tempfile::tempdir().unwrap();
    let missing = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from_path_buf(missing.path().join("project")).unwrap();
    let (memory, _) = memory_repository(&folder, &root).await;

    let expected = run(
        &Http::over(folder.open()),
        folder.dir.path().to_str().unwrap(),
    )
    .await;

    std::env::set_var("HOME", home.path());
    for name in ["TMPDIR", "TMP", "TEMP"] {
        std::env::set_var(name, temporary.path());
    }
    std::env::set_current_dir(working.path()).unwrap();
    let reads = InProcessReadOperations::new(memory.clone());
    let port = Counted::new(std::sync::Arc::new(reads));
    let memory = memory.with_read_operations(port.clone());
    let actual = run(&Http::over(memory), root.as_str()).await;

    assert_eq!(actual.answers.len(), expected.answers.len());
    let (mut over_memory, mut over_folder) = (Names::default(), Names::default());
    for (answer, folder_answer) in actual.answers.iter().zip(&expected.answers) {
        assert_eq!(
            compared(&mut over_memory, &actual.root, answer),
            compared(&mut over_folder, &expected.root, folder_answer),
            "{}",
            answer.shown
        );
    }
    assert_pinned(&actual.pinned());
    let runs = port.runs();
    assert_eq!(runs.len(), READS, "{runs:?}");
    assert_eq!(
        runs.iter().copied().collect::<BTreeSet<_>>(),
        READ_OPERATIONS.iter().copied().collect::<BTreeSet<_>>()
    );
    assert!(!root.exists(), "the router made the working tree");
    for directory in [&missing, &home, &working, &temporary] {
        assert_eq!(entries(directory.path()), Vec::<PathBuf>::new());
    }
}
