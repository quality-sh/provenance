//! The text calls that a record history read makes through an adapter that
//! an embedder gives.
//!
//! Implementation aid with no Rule binding: no Rule states how many texts a
//! history read asks for. The adapter counts its text calls, so these tests
//! do not need a Git process and run on each platform. A history read keeps
//! what it found for the last version list in the process, so these tests
//! are in their own binary, and the tests that read a history to its end
//! take `ONE_READER` so that no other test changes what the process keeps.

mod discussion_support;
use camino::Utf8Path;
use discussion_support::*;
use provenance_core::review::{ReviewHistoryPage, ReviewHistoryQuery};
use provenance_core::{NodeType, StableId};
use provenance_store::layout::ProvenanceLayout;
use provenance_store::operations::read_operations::{InProcessReadOperations, ReadOperations};
use provenance_store::operations::read_policy::ReadPolicy;
use provenance_store::operations::LocalRepository;
use provenance_store::repository::Repository;
use provenance_store::review::history::contract::{Commit, Entry, MemoryFileVersions};
use provenance_store::review::history::{BoxFuture, FileVersion, FileVersions};
use provenance_store::review::read_history;
use provenance_store::shards;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// The memory adapter, with a count of its text calls. When `same` is
/// true, it gives the same version list, the same `Arc`, each time for a
/// path; else it gives a new list on each call. The text of a commit in
/// `unavailable` is an error that is not a UTF-8 error.
#[derive(Debug)]
struct Counted {
    memory: MemoryFileVersions,
    same: bool,
    lists: Mutex<BTreeMap<String, Arc<Vec<FileVersion>>>>,
    texts: AtomicUsize,
    unavailable: Vec<String>,
}

/// Taken by each test that reads a history to its end.
static ONE_READER: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// The error of a text in `Counted::unavailable`.
const UNAVAILABLE: &str = "text store is unavailable";

impl Counted {
    fn new(commits: &[Commit]) -> Arc<Self> {
        Self::failing(commits, &[])
    }

    fn failing(commits: &[Commit], unavailable: &[&str]) -> Arc<Self> {
        Arc::new(Self {
            memory: MemoryFileVersions::new(commits),
            same: true,
            lists: Mutex::default(),
            texts: AtomicUsize::new(0),
            unavailable: unavailable
                .iter()
                .map(|&commit| commit.to_owned())
                .collect(),
        })
    }

    /// An adapter that gives a new version list on each call.
    fn new_lists(commits: &[Commit]) -> Arc<Self> {
        Arc::new(Self {
            memory: MemoryFileVersions::new(commits),
            same: false,
            lists: Mutex::default(),
            texts: AtomicUsize::new(0),
            unavailable: Vec::new(),
        })
    }

    /// The text calls since the last call of `calls`.
    fn calls(&self) -> usize {
        self.texts.swap(0, Ordering::SeqCst)
    }
}

impl FileVersions for Counted {
    fn versions<'a>(
        &'a self,
        path: &'a str,
    ) -> BoxFuture<'a, anyhow::Result<Arc<Vec<FileVersion>>>> {
        Box::pin(async move {
            let listed = self.memory.versions(path).await?;
            if !self.same {
                return Ok(listed);
            }
            let mut lists = self.lists.lock().unwrap();
            Ok(lists.entry(path.to_owned()).or_insert(listed).clone())
        })
    }

    fn text<'a>(
        &'a self,
        path: &'a str,
        commit: &'a str,
    ) -> BoxFuture<'a, anyhow::Result<Option<String>>> {
        self.texts.fetch_add(1, Ordering::SeqCst);
        if self.unavailable.iter().any(|name| name == commit) {
            return Box::pin(async { Err(anyhow::anyhow!(UNAVAILABLE)) });
        }
        self.memory.text(path, commit)
    }
}

/// The path of the Requirement records file below the working tree, and its
/// saved text.
fn records(temp: &tempfile::TempDir) -> (String, String) {
    let layout = ProvenanceLayout::new(Utf8Path::from_path(temp.path()).unwrap());
    let shard = shards::path_for(&layout, &scope(), NodeType::Requirement);
    let relative = shard
        .strip_prefix(Utf8Path::from_path(temp.path()).unwrap())
        .unwrap()
        .as_str()
        .replace('\\', "/");
    (relative, std::fs::read_to_string(&shard).unwrap())
}

/// A chain of commits `c0`, `c1`, … that each hold one entry at `path`.
fn chain(path: &str, entries: Vec<Entry>) -> Vec<Commit> {
    entries
        .into_iter()
        .enumerate()
        .map(|(index, entry)| {
            let name = format!("c{index}");
            let parent = index.checked_sub(1).map(|parent| format!("c{parent}"));
            let parents = parent.iter().map(String::as_str).collect::<Vec<_>>();
            Commit::new(&name, &parents, "ann", 1_700_000_000).with_entry(path, entry)
        })
        .collect()
}

async fn history(repository: &Repository, id: &str) -> anyhow::Result<ReviewHistoryPage> {
    let query = ReviewHistoryQuery {
        record_kind: NodeType::Requirement,
        record_id: StableId::new(id).unwrap(),
        limit: 50,
        cursor: None,
    };
    Ok(read_history(repository, ReadPolicy::default(), query)
        .await?
        .result)
}

/// Five versions of the records file: `c0` is the oldest. The text at each
/// index of `not_utf8` is not UTF-8.
fn versions(temp: &tempfile::TempDir, not_utf8: &[usize], unavailable: &[&str]) -> Arc<Counted> {
    let (relative, text) = records(temp);
    let entries = (0..5)
        .map(|index| {
            if not_utf8.contains(&index) {
                Entry::new(b"\xff\n".to_vec(), false)
            } else {
                Entry::text(&format!("{text}{index}\n"))
            }
        })
        .collect();
    Counted::failing(&chain(&relative, entries), unavailable)
}

/// The error of a history read of `req_a` through `versions`.
async fn read_error(temp: &tempfile::TempDir, versions: &Arc<Counted>) -> String {
    let root = Utf8Path::from_path(temp.path()).unwrap();
    let repository = repository(root).with_file_versions(versions.clone());
    let error = history(&repository, "req_a").await.unwrap_err();
    format!("{error:#}")
}

const NOT_UTF8: &str = "invalid utf-8 sequence of 1 bytes from index 0";

/// An error that is not a UTF-8 error ends the read at once: the read asks
/// for no text after it.
#[tokio::test]
async fn a_text_error_that_is_not_utf8_ends_the_read() {
    let (temp, _store) = fixture();
    let versions = versions(&temp, &[], &["c2"]);
    assert_eq!(read_error(&temp, &versions).await, UNAVAILABLE);
    assert_eq!(
        versions.calls(),
        3,
        "two versions before the error, and the error"
    );
}

/// After a text that is not UTF-8, the read asks for the remaining texts,
/// and a later error of another kind wins.
#[tokio::test]
async fn a_later_error_wins_over_a_text_that_is_not_utf8() {
    let (temp, _store) = fixture();
    let versions = versions(&temp, &[0], &["c3"]);
    assert_eq!(read_error(&temp, &versions).await, UNAVAILABLE);
    assert_eq!(versions.calls(), 4, "no text call after the later error");
}

/// With no later error of another kind, the first text that is not UTF-8
/// gives the error, after the read asks for each remaining text.
#[tokio::test]
async fn the_first_text_that_is_not_utf8_wins_with_no_later_error() {
    let (temp, _store) = fixture();
    let versions = versions(&temp, &[0, 2], &[]);
    assert_eq!(read_error(&temp, &versions).await, NOT_UTF8);
    assert_eq!(versions.calls(), 5, "each version");
}

/// A new read of the same version list reads no text, and a read of another
/// record of the same file does not take the kept answer.
#[tokio::test]
async fn a_new_read_of_the_same_versions_reads_no_text() {
    let _one = ONE_READER.lock().await;
    let (temp, store) = fixture();
    store
        .create_review_requirement(serde_json::from_value(json!({
            "actor":"ben","origin":null,
            "create":{"scope_id":"default","id":"req_b","statement":"The system lists records.","status":"discovery","depends_on":[],"supersedes":[]}
        }))
        .unwrap())
        .unwrap();
    let (relative, text) = records(&temp);
    let entries = (0..3)
        .map(|index| Entry::text(&format!("{text}{index}\n")))
        .collect();
    let versions = Counted::new(&chain(&relative, entries));
    let root = Utf8Path::from_path(temp.path()).unwrap();
    let repository = repository(root).with_file_versions(versions.clone());
    let answer = |page: ReviewHistoryPage| serde_json::to_value(page).unwrap();

    let first = answer(history(&repository, "req_a").await.unwrap());
    assert!(versions.calls() > 0, "the first read reads texts");
    let again = answer(history(&repository, "req_a").await.unwrap());
    assert_eq!(versions.calls(), 0, "a new read of the same versions");
    assert_eq!(again, first);

    let other = answer(history(&repository, "req_b").await.unwrap());
    assert!(versions.calls() > 0, "a read of another record reads texts");
    assert_ne!(other, first, "the revision of another record differs");

    let last = answer(history(&repository, "req_a").await.unwrap());
    assert_eq!(last, first);
}

/// `count` commits, each the child of the one before, that each give
/// `req_a` in the saved records file a new statement.
fn statement_edits(temp: &tempfile::TempDir, count: usize) -> Vec<Commit> {
    let (relative, text) = records(temp);
    let entries = (0..count)
        .map(|index| {
            let mut file = String::new();
            for line in text.lines() {
                let mut record: Value = serde_json::from_str(line).unwrap();
                if record["id"] == json!("req_a") {
                    record["statement"] = json!(format!("Edit {index} of the statement."));
                }
                file.push_str(&serde_json::to_string(&record).unwrap());
                file.push('\n');
            }
            Entry::text(&file)
        })
        .collect();
    chain(&relative, entries)
}

/// The answer of `review-history-entry` for the entry `entry` of `req_a`.
async fn history_entry(repository: &Repository, entry: &str) -> Value {
    let binding = LocalRepository::bind(repository.clone(), "folder").unwrap();
    let request = json!({
        "context": {"repository": "folder", "scope": "default"},
        "request": {"record_kind": "requirement", "record_id": "req_a", "entry_id": entry},
    });
    InProcessReadOperations::new(&binding)
        .run("review-history-entry", request)
        .await
        .unwrap()
}

/// The entry operation reads the history of the record once, also when the
/// entry is after the first page of 200 versions and the adapter gives a
/// new version list on each call: one text of the records file and one of
/// the Discussions file for each committed version.
#[tokio::test]
async fn an_entry_read_reads_the_history_once() {
    let _one = ONE_READER.lock().await;
    let mut counts = Vec::new();
    for committed in [205, 1_000] {
        let (temp, _store) = fixture();
        let versions = Counted::new_lists(&statement_edits(&temp, committed));
        let root = Utf8Path::from_path(temp.path()).unwrap();
        let repository = repository(root).with_file_versions(versions.clone());
        let answer = history_entry(&repository, "working").await;
        assert_eq!(answer["id"], json!("working"));
        counts.push((committed, versions.calls()));
    }
    assert_eq!(counts, [(205, 410), (1_000, 2_000)]);
}
