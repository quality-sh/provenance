//! The text calls that a record history read makes through an adapter that
//! an embedder gives.
//!
//! Implementation aid with no Rule binding: no Rule states how many texts a
//! history read asks for. The adapter counts its text calls, so these tests
//! do not need a Git process and run on each platform. A history read keeps
//! what it found for the last version list in the process, so these tests
//! are in their own binary.

mod discussion_support;
use camino::Utf8Path;
use discussion_support::*;
use provenance_core::review::{ReviewHistoryPage, ReviewHistoryQuery};
use provenance_core::{NodeType, StableId};
use provenance_store::layout::ProvenanceLayout;
use provenance_store::operations::read_policy::ReadPolicy;
use provenance_store::repository::Repository;
use provenance_store::review::history::contract::{Commit, Entry, MemoryFileVersions};
use provenance_store::review::history::{BoxFuture, FileVersion, FileVersions};
use provenance_store::review::read_history;
use provenance_store::shards;
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// The memory adapter, with a count of its text calls. It gives the same
/// version list, the same `Arc`, each time for a path.
#[derive(Debug)]
struct Counted {
    memory: MemoryFileVersions,
    lists: Mutex<BTreeMap<String, Arc<Vec<FileVersion>>>>,
    texts: AtomicUsize,
}

impl Counted {
    fn new(commits: &[Commit]) -> Arc<Self> {
        Arc::new(Self {
            memory: MemoryFileVersions::new(commits),
            lists: Mutex::default(),
            texts: AtomicUsize::new(0),
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

/// Only a Git failure of the git adapter can win over an earlier text error,
/// so the read asks another adapter for no text after an error.
#[tokio::test]
async fn a_text_error_of_another_adapter_ends_the_read() {
    let (temp, _store) = fixture();
    let (relative, text) = records(&temp);
    let mut entries = vec![Entry::new(b"\xff\n".to_vec(), false)];
    entries.extend((1..5).map(|index| Entry::text(&format!("{text}{index}\n"))));
    let versions = Counted::new(&chain(&relative, entries));
    let root = Utf8Path::from_path(temp.path()).unwrap();
    let repository = repository(root).with_file_versions(versions.clone());
    let error = history(&repository, "req_a").await.unwrap_err();
    assert_eq!(
        format!("{error:#}"),
        "invalid utf-8 sequence of 1 bytes from index 0"
    );
    assert_eq!(versions.calls(), 1, "text calls after the error");
}

/// A new read of the same version list reads no text, and a read of another
/// record of the same file does not take the kept answer.
#[tokio::test]
async fn a_new_read_of_the_same_versions_reads_no_text() {
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
    assert_ne!(other, first);
    assert!(other.to_string().contains("req_b"), "{other}");

    let last = answer(history(&repository, "req_a").await.unwrap());
    assert_eq!(last, first);
}
