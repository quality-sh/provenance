//! The history operation over a version larger than the page bytes: a
//! client that follows `next_cursor` comes to the end of the history.

use super::{fixture, repository, statement_edits, MemoryFileVersions, ONE_READER};
use crate::history_pages::follow;
use camino::Utf8Path;
use provenance_macros::verifies;
use provenance_store::operations::read_operations::InProcessReadOperations;
use provenance_store::operations::LocalRepository;
use serde_json::{json, Value};
use std::sync::Arc;

/// The ids of the entries of each page of `review-history` for `req_a`,
/// over three commits that change `req_a`, where the author name of `c1`
/// has `author` bytes. The read follows `next_cursor` from the first page.
/// A failed page gives the pages before it and its error.
async fn pages(author: usize) -> (Vec<Vec<String>>, Option<Value>) {
    let _one = ONE_READER.lock().await;
    let (temp, _store) = fixture();
    let mut commits = statement_edits(&temp, 3);
    commits[1].author = "a".repeat(author);
    let root = Utf8Path::from_path(temp.path()).unwrap();
    let repository =
        repository(root).with_file_versions(Arc::new(MemoryFileVersions::new(&commits)));
    let binding = LocalRepository::bind(repository, "folder").unwrap();
    let operations = InProcessReadOperations::new(&binding);
    follow(&operations, str::to_owned).await
}

/// A version larger than the page bytes and smaller than the response
/// budget is alone on its page.
///
/// Implementation aid, no Rule: no Rule in the graph says a page with a cursor holds an entry.
#[tokio::test]
async fn a_version_larger_than_a_page_is_alone_on_its_page() {
    assert_eq!(
        pages(1_050_000).await,
        (
            vec![
                vec!["c0".to_owned()],
                vec!["c1".to_owned()],
                vec!["c2".to_owned(), "working".to_owned()],
            ],
            None
        )
    );
}

/// A version larger than the response budget gives the explicit budget
/// refusal on page 2, its page, and page 1 gives the version before it.
#[tokio::test]
#[verifies("rule_query_pages_bound_shared_reads", examples)]
async fn a_version_larger_than_the_budget_gives_page_budget_exceeded() {
    assert_eq!(
        pages(1_200_000).await,
        (
            vec![vec!["c0".to_owned()]],
            Some(json!({"kind": "page_budget_exceeded"}))
        )
    );
}
