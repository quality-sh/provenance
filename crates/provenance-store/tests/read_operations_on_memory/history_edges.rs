//! The history operations over two histories that only the memory adapter
//! holds, with and without `host`:
//!
//! - The history of `history_fixture.rs` over an adapter that lists the
//!   commit `bob` but does not hold it: each text at `bob` is `None`, as at
//!   the edge of a shallow clone. `bob` is on the lineage of the Rule, so
//!   the Rule cases, which read it, differ from the cases over the whole
//!   history.
//! - A history of 205 commits that each change the statement of `req_a`,
//!   so that the entry operation reads past the first page of 200 versions.
//!
//! The answers, without their stamps, equal a checked file.
//!
//! Implementation aid with no Rule binding: these tests pin the answers of
//! the history operations at these edges so that a change of the path they
//! read through cannot change them.

use super::history::{assert_pinned, fixture, memory_repository, MemoryReads};
use crate::history_cases::{answers, run, CASES, ENTRY};
use crate::history_long::{long_history, long_name, LONG};
use provenance_store::review::history::contract::MemoryFileVersions;
use provenance_store::review::history::{BoxFuture, FileVersion, FileVersions};
use serde_json::json;
use std::sync::Arc;

/// The checked answers of the edges.
const PINNED: &str = include_str!("../fixtures/history_edges.json");

/// File versions that list the commits of `held` but do not hold the
/// commit `missing`: its text is `None` at each path.
#[derive(Debug)]
struct Missing {
    held: MemoryFileVersions,
    missing: &'static str,
}

impl FileVersions for Missing {
    fn versions<'a>(
        &'a self,
        path: &'a str,
    ) -> BoxFuture<'a, anyhow::Result<Arc<Vec<FileVersion>>>> {
        self.held.versions(path)
    }

    fn text<'a>(
        &'a self,
        path: &'a str,
        commit: &'a str,
    ) -> BoxFuture<'a, anyhow::Result<Option<String>>> {
        if commit == self.missing {
            return Box::pin(std::future::ready(Ok(None)));
        }
        self.held.text(path, commit)
    }
}

#[tokio::test]
async fn the_history_operations_at_the_edges_give_the_checked_answers() {
    let history = fixture();
    let missing = || {
        Some(Arc::new(Missing {
            held: MemoryFileVersions::new(&history.commits),
            missing: "bob",
        }) as _)
    };
    let reads = MemoryReads {
        default: memory_repository(&history, "default", missing()).await,
        broken: memory_repository(&history, "broken", missing()).await,
    };
    let rules: Vec<_> = CASES
        .into_iter()
        .filter(|case| case.kind == "rule")
        .collect();
    let mut read = run(&reads, &rules).await;
    let whole = || Some(Arc::new(MemoryFileVersions::new(&history.commits)) as _);
    let reads = MemoryReads {
        default: memory_repository(&history, "default", whole()).await,
        broken: memory_repository(&history, "broken", whole()).await,
    };
    assert_ne!(read, run(&reads, &rules).await);

    let long = || Some(Arc::new(MemoryFileVersions::new(&long_history(&history.working))) as _);
    let reads = MemoryReads {
        default: memory_repository(&history, "default", long()).await,
        broken: memory_repository(&history, "broken", long()).await,
    };
    let entries = [
        long_name(0),
        long_name(199),
        long_name(200),
        long_name(LONG - 1),
        "working".to_owned(),
        "missing".to_owned(),
    ];
    let requests = entries
        .iter()
        .map(|entry| json!({"record_kind": "requirement", "record_id": "req_a", "entry_id": entry}))
        .collect();
    read.extend(answers(&reads, ENTRY, "default", requests).await);
    assert_pinned(PINNED, &serde_json::to_string_pretty(&read).unwrap());
}
