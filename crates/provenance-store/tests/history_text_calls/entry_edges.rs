//! The entry operation at the edges of a page of 200 versions: a version
//! larger than the page bytes, a version larger than the response budget,
//! and a commit that lands while the read lists the versions.
//!
//! Implementation aid with no Rule binding: no Rule states how an entry
//! read pages. Each read has a time limit, so a read that does not end
//! fails the test.

use super::{
    fixture, history_entry, records, repository, statement_edits, Counted, ErasedFailure,
    ONE_READER,
};
use camino::Utf8Path;
use provenance_store::review::history::contract::{Commit, Entry, MemoryFileVersions};
use provenance_store::review::history::{BoxFuture, FileVersion, FileVersions};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// The time limit of one entry read.
const LIMIT: Duration = Duration::from_secs(60);

/// An author name larger than the page bytes of a history page
/// (1,032,192) and smaller than the response budget (1,114,112).
const OVER_A_PAGE: usize = 1_050_000;

/// An author name larger than the response budget.
const OVER_THE_BUDGET: usize = 1_200_000;

/// The answer of the entry operation for `entry` of `req_a`, over three
/// commits that change `req_a`, where the author name of `c1` has
/// `author` bytes.
async fn entry_after_a_large_author(author: usize, entry: &str) -> Result<Value, ErasedFailure> {
    let _one = ONE_READER.lock().await;
    let (temp, _store) = fixture();
    let mut commits = statement_edits(&temp, 3);
    commits[1].author = "a".repeat(author);
    let root = Utf8Path::from_path(temp.path()).unwrap();
    let repository = repository(root).with_file_versions(Counted::new(&commits));
    tokio::time::timeout(LIMIT, history_entry(&repository, entry))
        .await
        .expect("the entry read ends")
}

/// A version larger than the page bytes of a history page is in the
/// answer of the entry operation when it is in the response budget.
#[tokio::test]
async fn a_version_larger_than_a_page_gives_the_entry() {
    let answer = entry_after_a_large_author(OVER_A_PAGE, "c1").await.unwrap();
    assert_eq!(answer["id"], json!("c1"));
    assert_eq!(answer["author"].as_str().map(str::len), Some(OVER_A_PAGE));
}

/// A version larger than the response budget gives `page_budget_exceeded`,
/// and the entry after it is in the answer.
#[tokio::test]
async fn a_version_larger_than_the_budget_gives_page_budget_exceeded() {
    let refused = entry_after_a_large_author(OVER_THE_BUDGET, "c1")
        .await
        .unwrap_err();
    assert_eq!(refused.error, json!({"kind": "page_budget_exceeded"}));
    let after = entry_after_a_large_author(OVER_THE_BUDGET, "c2")
        .await
        .unwrap();
    assert_eq!(after["id"], json!("c2"));
}

/// File versions that give the commits of `before` on the first versions
/// call, and the commits of `after` on each later call: a commit lands
/// after the first list.
#[derive(Debug)]
struct Landing {
    before: MemoryFileVersions,
    after: MemoryFileVersions,
    calls: AtomicUsize,
}

impl FileVersions for Landing {
    fn versions<'a>(
        &'a self,
        path: &'a str,
    ) -> BoxFuture<'a, anyhow::Result<Arc<Vec<FileVersion>>>> {
        if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
            return self.before.versions(path);
        }
        self.after.versions(path)
    }

    fn text<'a>(
        &'a self,
        path: &'a str,
        commit: &'a str,
    ) -> BoxFuture<'a, anyhow::Result<Option<String>>> {
        self.after.text(path, commit)
    }
}

/// A commit that lands while the entry operation reads gives no mixed
/// answer: the entry `working`, after the first page of 200 versions, is
/// the entry over the versions before the commit. The commit holds the
/// saved records file, so the versions after it have no `working`.
#[tokio::test]
async fn a_commit_that_lands_during_the_read_gives_one_answer() {
    let _one = ONE_READER.lock().await;
    let (temp, _store) = fixture();
    let before = statement_edits(&temp, 205);
    let (path, text) = records(&temp);
    let mut after = before.clone();
    after.push(
        Commit::new("c205", &["c204"], "ann", 1_700_000_000).with_entry(&path, Entry::text(&text)),
    );
    let root = Utf8Path::from_path(temp.path()).unwrap();
    let still = repository(root).with_file_versions(Arc::new(MemoryFileVersions::new(&before)));
    let expected = history_entry(&still, "working").await.unwrap();
    let landing = Arc::new(Landing {
        before: MemoryFileVersions::new(&before),
        after: MemoryFileVersions::new(&after),
        calls: AtomicUsize::new(0),
    });
    let repository = repository(root).with_file_versions(landing);
    let answer = tokio::time::timeout(LIMIT, history_entry(&repository, "working"))
        .await
        .expect("the entry read ends");
    assert_eq!(answer.map_err(|failure| failure.error), Ok(expected));
}
