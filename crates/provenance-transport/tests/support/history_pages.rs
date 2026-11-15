//! The long history over memory storage through the contract router, and a
//! client that follows the cursor of its history route.

use super::history_cases::LIST;
use super::history_fixture::{history, History, Sources};
use super::history_long::long_history;
use super::history_routes::path;
use super::memory_host::{bound, in_process};
use super::router_answer::Http;
use camino::Utf8PathBuf;
use provenance_store::repository::{RandomIds, Repository as Opened, SystemClock};
use serde_json::{json, Value};
use std::sync::Arc;
use tempfile::TempDir;

/// The most pages that one read of a history follows.
const FOLLOWS: usize = 10;

/// The long history of `req_a` with the working copy of the history
/// fixture, and a project root that does not exist below a new temporary
/// directory.
pub fn long_setup() -> (History, TempDir, Utf8PathBuf) {
    let base = history(&Sources {
        clock: Arc::new(SystemClock),
        ids: Arc::new(RandomIds),
    });
    let long = History {
        commits: long_history(&base.working),
        working: base.working,
    };
    let missing = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from_path_buf(missing.path().join("project")).unwrap();
    (long, missing, root)
}

/// A client of the contract router over `repository`, through its counted
/// in-process port.
pub fn over_port(repository: Opened) -> Http {
    let port = in_process(&repository);
    Http::with_access(Arc::new(bound(repository, port)))
}

/// The ids of the entries of each page of the history route for `req_a`
/// through `http`. The client follows `next_cursor` from the first page. A
/// failed page gives the pages before it, and its status and its error.
pub async fn follow(http: &Http) -> (Vec<Vec<String>>, Option<(u16, Value)>) {
    let mut request = json!({"record_kind": "requirement", "record_id": "req_a", "limit": 200});
    let mut pages = Vec::new();
    for _ in 0..FOLLOWS {
        let route = path(LIST, &request);
        let answer = http.call("GET", &route, None, &[], route.clone()).await;
        let body = answer.json();
        if answer.status != 200 {
            return (pages, Some((answer.status, body["error"].clone())));
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
            return (pages, None);
        }
        request["cursor"] = next.clone();
    }
    panic!("the history has more than {FOLLOWS} pages");
}
