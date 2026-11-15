//! The history route of a Requirement through the contract router, over
//! memory storage with the history contract's memory adapter, when one
//! version is larger than the page bytes: a client that follows
//! `next_cursor` comes to the end of the history.

use crate::support::history_cases::LIST;
use crate::support::history_fixture::{history, Sources};
use crate::support::history_long::{long_history, long_name, LONG};
use crate::support::history_routes::path;
use crate::support::memory_host::{bound, in_process, over_memory};
use crate::support::router_answer::Http;
use camino::Utf8PathBuf;
use provenance_macros::verifies;
use provenance_store::repository::{RandomIds, SystemClock};
use provenance_store::review::history::contract::MemoryFileVersions;
use provenance_store::settings::Settings;
use serde_json::{json, Value};
use std::sync::Arc;

/// The most pages that one read of a history follows.
const FOLLOWS: usize = 10;

/// The ids of the entries of each page of the history of `req_a` over the
/// 205-commit history, where the author name of `edit_001` has `author`
/// bytes. The client follows `next_cursor` from the first page. A failed
/// page gives its status and its error.
async fn pages(author: usize) -> Result<Vec<Vec<String>>, (u16, Value)> {
    let base = history(&Sources {
        clock: Arc::new(SystemClock),
        ids: Arc::new(RandomIds),
    });
    let mut commits = long_history(&base.working);
    commits[1].author = "a".repeat(author);
    let missing = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from_path_buf(missing.path().join("project")).unwrap();
    let (repository, _) = over_memory(&root, "default", Settings::default(), base.state()).await;
    let repository = repository.with_file_versions(Arc::new(MemoryFileVersions::new(&commits)));
    let port = in_process(&repository);
    let http = Http::with_access(Arc::new(bound(repository, port)));
    let mut request = json!({"record_kind": "requirement", "record_id": "req_a", "limit": 200});
    let mut pages = Vec::new();
    for _ in 0..FOLLOWS {
        let route = path(LIST, &request);
        let answer = http.call("GET", &route, None, &[], route.clone()).await;
        let body = answer.json();
        if answer.status != 200 {
            return Err((answer.status, body["error"].clone()));
        }
        let ids: Vec<String> = body["data"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| entry["id"].as_str().unwrap().to_owned())
            .collect();
        let next = &body["meta"]["next_cursor"];
        assert!(
            !(ids.is_empty() && next.is_string()),
            "page {} is empty and gives a cursor; the cursor it was read with: {}",
            pages.len() + 1,
            request["cursor"]
        );
        pages.push(ids);
        if !next.is_string() {
            return Ok(pages);
        }
        request["cursor"] = next.clone();
    }
    panic!("the history has more than {FOLLOWS} pages");
}

/// A version larger than the page bytes and smaller than the response
/// budget is alone on its page, and the pages after it hold the rest of
/// the history.
#[tokio::test]
#[verifies("rule_query_pages_bound_shared_reads", examples)]
async fn a_version_larger_than_a_page_is_alone_on_its_page() {
    let _alone = crate::one_at_a_time().await;
    let pages = pages(1_050_000).await.unwrap();
    let sizes: Vec<usize> = pages.iter().map(Vec::len).collect();
    assert_eq!(sizes, [1, 1, 200, 4]);
    assert_eq!(pages[0], [long_name(0)]);
    assert_eq!(pages[1], [long_name(1)]);
    assert_eq!(pages[2].first(), Some(&long_name(2)));
    assert_eq!(pages[3].last().map(String::as_str), Some("working"));
    assert_eq!(pages[3][2], long_name(LONG - 1));
}

/// A version larger than the response budget gives the explicit budget
/// refusal on its page.
#[tokio::test]
#[verifies("rule_query_pages_bound_shared_reads", examples)]
async fn a_version_larger_than_the_budget_gives_page_budget_exceeded() {
    let _alone = crate::one_at_a_time().await;
    assert_eq!(
        pages(1_200_000).await,
        Err((409, json!({"kind": "page_budget_exceeded"})))
    );
}
