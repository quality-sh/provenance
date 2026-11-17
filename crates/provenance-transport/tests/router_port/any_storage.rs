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

use crate::support::history_cases::{CASES, ENTRY, EVIDENCE, LIST};
use crate::support::history_fixture::{history, History, Sources};
use crate::support::history_folder::{folder, Folder};
use crate::support::history_long::edits;
use crate::support::history_pages::{long_setup, over_port};
use crate::support::history_routes::{self, history_routes, path, route_name};
use crate::support::memory_host::{bound, compared, in_process, memory_repository, over_memory};
use crate::support::names::Names;
use crate::support::records::{allow_reviewer, Repository};
use crate::support::router_answer::Http;
use crate::support::site_operations::{assert_pinned, run};
use camino::{Utf8Path, Utf8PathBuf};
use provenance_store::operations::read_operations::READ_OPERATIONS;
use provenance_store::repository::{RandomIds, Repository as Opened, SystemClock};
use provenance_store::review::history::contract::MemoryFileVersions;
use provenance_store::review::history::{BoxFuture, FileVersion, FileVersions};
use provenance_store::settings::Settings;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// The read operations of the history routes, which the second test calls.
const HISTORY: [&str; 3] = ["review-evidence", "review-history", "review-history-entry"];

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
        READ_OPERATIONS
            .iter()
            .copied()
            .filter(|name| !HISTORY.contains(name))
            .collect::<BTreeSet<_>>()
    );
    assert!(!root.exists(), "the router made the working tree");
    for directory in [&missing, &home, &working, &temporary] {
        assert_eq!(entries(directory.path()), Vec::<PathBuf>::new());
    }
}

/// The repository of `scope` over memory storage that holds the working
/// copy of `history`, at the working tree `root`, with the history
/// contract's memory adapter of the commits of `folder`, under their Git
/// ids.
pub async fn history_in_memory(
    history: &History,
    folder: &Folder,
    root: &Utf8Path,
    scope: &str,
) -> Opened {
    history_in_memory_with(history, folder, root, scope, |memory| Arc::new(memory)).await
}

/// The repository of [`history_in_memory`], with the adapter that `adapter`
/// makes of the history contract's memory adapter.
async fn history_in_memory_with(
    history: &History,
    folder: &Folder,
    root: &Utf8Path,
    scope: &str,
    adapter: impl FnOnce(MemoryFileVersions) -> Arc<dyn FileVersions>,
) -> Opened {
    let ids: BTreeMap<&str, &str> = folder
        .names
        .iter()
        .map(|(id, name)| (name.as_str(), id.as_str()))
        .collect();
    let commits: Vec<_> = history
        .commits
        .iter()
        .map(|commit| {
            let mut commit = commit.clone();
            commit.name = ids[commit.name.as_str()].to_owned();
            for parent in &mut commit.parents {
                *parent = ids[parent.as_str()].to_owned();
            }
            commit
        })
        .collect();
    let (repository, _) = over_memory(root, scope, Settings::default(), history.state()).await;
    repository.with_file_versions(adapter(MemoryFileVersions::new(&commits)))
}

/// The history contract's memory adapter, with a count of its text calls.
#[derive(Debug)]
struct Counted {
    memory: MemoryFileVersions,
    texts: AtomicUsize,
}

impl FileVersions for Counted {
    fn versions<'a>(
        &'a self,
        path: &'a str,
    ) -> BoxFuture<'a, anyhow::Result<Arc<Vec<FileVersion>>>> {
        self.memory.versions(path)
    }

    fn text<'a>(
        &'a self,
        path: &'a str,
        commit: &'a str,
    ) -> BoxFuture<'a, anyhow::Result<Option<String>>> {
        self.texts.fetch_add(1, Ordering::SeqCst);
        self.memory.text(path, commit)
    }
}

/// The 18 history routes over memory storage, through the port, with the
/// history contract's memory adapter of a folder's Git history, give the
/// answers of the router over the folder, whose binding has no port.
#[tokio::test]
async fn the_history_routes_over_memory_storage_answer_as_over_a_folder() {
    let _alone = crate::one_at_a_time().await;
    let mut routes = history_routes();
    routes.sort_unstable();
    let mut cases: Vec<_> = CASES.iter().map(route_name).collect();
    cases.sort_unstable();
    assert_eq!(cases, routes, "each history route has one case");
    let history = history(&Sources {
        clock: Arc::new(SystemClock),
        ids: Arc::new(RandomIds),
    });
    // Each scope has a folder of its own, so that each folder host has its
    // own read database, as each memory host has. The commits are the same,
    // so their ids are too.
    let broken = folder(&history);
    let folder = folder(&history);
    assert_eq!(broken.names, folder.names);
    let home = tempfile::tempdir().unwrap();
    let working = tempfile::tempdir().unwrap();
    let temporary = tempfile::tempdir().unwrap();
    let missing = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from_path_buf(missing.path().join("project")).unwrap();
    let memory = history_in_memory(&history, &folder, &root, "default").await;
    let memory_broken = history_in_memory(&history, &folder, &root, "broken").await;

    let opened = |folder: &Folder, scope| Http::over(Opened::open(folder.root(), scope).unwrap());
    let expected =
        history_routes::run(&opened(&folder, "default"), &opened(&broken, "broken")).await;

    let _saved = Saved::now();
    std::env::set_var("HOME", home.path());
    for name in ["TMPDIR", "TMP", "TEMP"] {
        std::env::set_var(name, temporary.path());
    }
    std::env::set_current_dir(working.path()).unwrap();
    let port = in_process(&memory);
    let port_broken = in_process(&memory_broken);
    let actual = history_routes::run(
        &Http::with_access(Arc::new(bound(memory, port.clone()))),
        &Http::with_access(Arc::new(bound(memory_broken, port_broken.clone()))),
    )
    .await;

    assert_eq!(actual.len(), expected.len());
    let (mut over_memory, mut over_folder) = (Names::default(), Names::default());
    for (answer, folder_answer) in actual.iter().zip(&expected) {
        assert_eq!(
            compared(&mut over_memory, answer),
            compared(&mut over_folder, folder_answer),
            "{}",
            folder.named(&answer.shown)
        );
    }
    let runs: BTreeSet<_> = port.runs().into_iter().chain(port_broken.runs()).collect();
    assert_eq!(runs, BTreeSet::from(HISTORY));
    assert!(!root.exists(), "the router made the working tree");
    for directory in [&missing, &home, &working, &temporary] {
        assert_eq!(entries(directory.path()), Vec::<PathBuf>::new());
    }
}

/// The three history routes over a record file with more than 1,000
/// versions to index give the budget refusal once, over memory storage
/// through the port as over a folder's Git history. Then the routes give
/// the same answers over both. The index of the file stays in the read
/// database in memory, so the reads after it read no text.
#[tokio::test]
async fn the_history_routes_index_more_than_1000_file_versions_as_over_a_folder() {
    let _alone = crate::one_at_a_time().await;
    let (base, _missing, root) = long_setup();
    let long = History {
        commits: edits(&base.working, 1_001),
        working: base.working,
    };
    let folder = folder(&long);
    let counted = Arc::new(std::sync::OnceLock::<Arc<Counted>>::new());
    let kept = counted.clone();
    let memory = history_in_memory_with(&long, &folder, &root, "default", move |memory| {
        let counted = Arc::new(Counted {
            memory,
            texts: AtomicUsize::new(0),
        });
        kept.set(counted.clone()).unwrap();
        counted
    })
    .await;
    let texts = || counted.get().unwrap().texts.swap(0, Ordering::SeqCst);
    let over_memory = over_port(memory);
    let over_folder = Http::over(Opened::open(folder.root(), "default").unwrap());
    let record = json!({"record_kind": "requirement", "record_id": "req_a"});
    let mut entry = record.clone();
    entry["entry_id"] = json!("working");
    let mut evidence = entry.clone();
    evidence["side"] = json!("after");
    let routes = [
        path(LIST, &record),
        path(ENTRY, &entry),
        path(EVIDENCE, &evidence),
    ];
    for http in [&over_memory, &over_folder] {
        let answer = http
            .call("GET", &routes[0], None, &[], routes[0].clone())
            .await;
        assert_eq!(
            (answer.status, answer.json()["error"].clone()),
            (409, json!({"kind": "page_budget_exceeded"}))
        );
    }
    assert_eq!(texts(), 2_000, "one read indexes 1,000 versions");
    let (mut memory_names, mut folder_names) = (Names::default(), Names::default());
    for round in 0..2 {
        for route in &routes {
            let answer = over_memory
                .call("GET", route, None, &[], route.clone())
                .await;
            let expected = over_folder
                .call("GET", route, None, &[], route.clone())
                .await;
            assert_eq!(expected.status, 200, "{}", folder.named(route));
            assert_eq!(
                compared(&mut memory_names, &answer),
                compared(&mut folder_names, &expected),
                "{}",
                folder.named(route)
            );
        }
        assert_eq!(texts(), if round == 0 { 2 } else { 0 }, "round {round}");
    }
}
