//! The history route of a Requirement through the contract router, over
//! memory storage with the history contract's memory adapter, when one
//! version is larger than the page bytes: a client that follows
//! `next_cursor` comes to the end of the history.

use crate::support::history_long::{long_name, LONG};
use crate::support::history_pages::{follow, long_setup, over_port};
use crate::support::memory_host::over_memory;
use provenance_macros::verifies;
use provenance_store::review::history::contract::MemoryFileVersions;
use provenance_store::settings::Settings;
use serde_json::{json, Value};
use std::sync::Arc;

/// The ids of the entries of each page of the history of `req_a` over the
/// 205-commit history, where the author name of `edit_001` has `author`
/// bytes. The client follows `next_cursor` from the first page. A failed
/// page gives the pages before it, and its status and its error.
async fn pages(author: usize) -> (Vec<Vec<String>>, Option<(u16, Value)>) {
    let (mut long, _missing, root) = long_setup();
    long.commits[1].author = "a".repeat(author);
    let (repository, _) = over_memory(&root, "default", Settings::default(), long.state()).await;
    let repository =
        repository.with_file_versions(Arc::new(MemoryFileVersions::new(&long.commits)));
    follow(&over_port(repository)).await
}

/// A version larger than the page bytes and smaller than the response
/// budget is alone on its page, and the pages after it hold the rest of
/// the history.
///
/// Implementation aid, no Rule: no Rule in the graph says a page with a cursor holds an entry.
#[tokio::test]
async fn a_version_larger_than_a_page_is_alone_on_its_page() {
    let _alone = crate::one_at_a_time().await;
    let (pages, failure) = pages(1_050_000).await;
    assert_eq!(failure, None);
    let sizes: Vec<usize> = pages.iter().map(Vec::len).collect();
    assert_eq!(sizes, [1, 1, 200, 4]);
    assert_eq!(pages[0], [long_name(0)]);
    assert_eq!(pages[1], [long_name(1)]);
    assert_eq!(pages[2].first(), Some(&long_name(2)));
    assert_eq!(pages[3].last().map(String::as_str), Some("working"));
    assert_eq!(pages[3][2], long_name(LONG - 1));
}

/// A version larger than the response budget gives the explicit budget
/// refusal on page 2, its page, and page 1 gives the version before it.
#[tokio::test]
#[verifies("rule_query_pages_bound_shared_reads", examples)]
async fn a_version_larger_than_the_budget_gives_page_budget_exceeded() {
    let _alone = crate::one_at_a_time().await;
    assert_eq!(
        pages(1_200_000).await,
        (
            vec![vec![long_name(0)]],
            Some((409, json!({"kind": "page_budget_exceeded"})))
        )
    );
}
