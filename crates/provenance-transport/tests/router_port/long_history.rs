//! The entry route past the first page of 200 versions over memory storage,
//! through the port, with the history contract's memory adapter of a
//! folder's Git history, gives the answers of the router over the folder.
//!
//! No Rule: this test is an implementation aid, as `any_storage` is.

use crate::any_storage::history_in_memory;
use crate::support::history_cases::ENTRY;
use crate::support::history_fixture::{history, History, Sources};
use crate::support::history_folder::folder;
use crate::support::history_long::{long_history, long_name, LONG};
use crate::support::history_routes::path;
use crate::support::memory_host::{bound, compared, in_process};
use crate::support::names::Names;
use crate::support::router_answer::Http;
use camino::Utf8PathBuf;
use provenance_store::repository::{RandomIds, Repository as Opened, SystemClock};
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::Arc;

/// The entries of `req_a` at the edges of the first page, the last commit,
/// the working copy and an entry that does not exist, with the status of
/// each answer.
#[tokio::test]
async fn the_entry_route_past_a_page_answers_over_memory_as_over_git() {
    let _alone = crate::one_at_a_time().await;
    let base = history(&Sources {
        clock: Arc::new(SystemClock),
        ids: Arc::new(RandomIds),
    });
    let long = History {
        commits: long_history(&base.working),
        working: base.working.clone(),
    };
    let folder = folder(&long);
    let missing = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from_path_buf(missing.path().join("project")).unwrap();
    let memory = history_in_memory(&long, &folder, &root, "default").await;
    let port = in_process(&memory);
    let over_memory = Http::with_access(Arc::new(bound(memory, port)));
    let over_folder = Http::over(Opened::open(folder.root(), "default").unwrap());
    let ids: BTreeMap<&str, &str> = folder
        .names
        .iter()
        .map(|(id, name)| (name.as_str(), id.as_str()))
        .collect();
    let (mut memory_names, mut folder_names) = (Names::default(), Names::default());
    let entries = [
        (long_name(199), 200),
        (long_name(200), 200),
        (long_name(LONG - 1), 200),
        ("working".to_owned(), 200),
        ("missing".to_owned(), 404),
    ];
    for (entry, status) in entries {
        let entry = ids.get(entry.as_str()).copied().unwrap_or(entry.as_str());
        let request =
            json!({"record_kind": "requirement", "record_id": "req_a", "entry_id": entry});
        let route = path(ENTRY, &request);
        let answer = over_memory
            .call("GET", &route, None, &[], route.clone())
            .await;
        let expected = over_folder
            .call("GET", &route, None, &[], route.clone())
            .await;
        assert_eq!(expected.status, status, "{}", folder.named(&route));
        assert_eq!(
            compared(&mut memory_names, &answer),
            compared(&mut folder_names, &expected),
            "{}",
            folder.named(&route)
        );
    }
}
