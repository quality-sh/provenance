//! The contract router over a repository whose state is in memory gives the
//! answers of the router over the same state in a folder.
//!
//! The repository in memory has a working tree that does not exist and a
//! read database in memory, and its binding runs its read operations through
//! the in-process port. The home directory, the working directory and the
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
//! the owner's review. This test is an implementation aid.

use crate::support::memory_host::{bound, compared, in_process, memory_repository};
use crate::support::names::Names;
use crate::support::records::{allow_reviewer, Repository};
use crate::support::router_answer::Http;
use crate::support::site_operations::{assert_pinned, run};
use camino::Utf8PathBuf;
use provenance_store::operations::read_operations::READ_OPERATIONS;
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The number of calls of `run` to read operations: the 25 reads, the read
/// of the new Requirement, and each record again at the end.
const READS: usize = 25 + 1 + 6;

/// The home, temporary and working directories of the process, set back
/// when dropped, so the next test of this binary in the same process makes
/// its directories where it did.
struct Saved {
    variables: Vec<(&'static str, Option<OsString>)>,
    working: PathBuf,
}

impl Saved {
    fn now() -> Self {
        Self {
            variables: ["HOME", "TMPDIR", "TMP", "TEMP"]
                .map(|name| (name, std::env::var_os(name)))
                .into(),
            working: std::env::current_dir().unwrap(),
        }
    }
}

impl Drop for Saved {
    fn drop(&mut self) {
        for (name, value) in &self.variables {
            match value {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }
        std::env::set_current_dir(&self.working).unwrap();
    }
}

/// The entries of `directory`.
fn entries(directory: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect()
}

#[tokio::test]
async fn the_router_over_memory_storage_answers_as_over_a_folder() {
    let _alone = crate::one_at_a_time().await;
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

    let _saved = Saved::now();
    std::env::set_var("HOME", home.path());
    for name in ["TMPDIR", "TMP", "TEMP"] {
        std::env::set_var(name, temporary.path());
    }
    std::env::set_current_dir(working.path()).unwrap();
    let port = in_process(&memory);
    let binding = bound(memory, port.clone());
    let actual = run(&Http::with_access(Arc::new(binding)), root.as_str()).await;

    assert_eq!(actual.answers.len(), expected.answers.len());
    let (mut over_memory, mut over_folder) = (Names::default(), Names::default());
    for (answer, folder_answer) in actual.answers.iter().zip(&expected.answers) {
        assert_eq!(
            compared(&mut over_memory, answer),
            compared(&mut over_folder, folder_answer),
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
