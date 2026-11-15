//! The history operations over two histories that only the memory adapter
//! holds, with and without `host`:
//!
//! - The history of `history_fixture.rs` over an adapter that lists the
//!   commit `alice` but does not hold it: each text at `alice` is `None`, as
//!   at the edge of a shallow clone. The Rule cases read it.
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
use crate::history_fixture::{Tree, STATE};
use provenance_store::review::history::contract::{Commit, MemoryFileVersions};
use provenance_store::review::history::{BoxFuture, FileVersion, FileVersions};
use serde_json::{json, Value};
use std::sync::Arc;

/// The checked answers of the edges.
const PINNED: &str = include_str!("../fixtures/history_edges.json");

/// The commits of the long history.
const LONG: i64 = 205;

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

/// The name of commit `index` of the long history.
fn long_name(index: i64) -> String {
    format!("edit_{index:03}")
}

/// `LONG` commits, each the child of the one before, that each give
/// `req_a` in the Requirement file of `default` in `working` a new
/// statement.
fn long_history(working: &Tree) -> Vec<Commit> {
    let path = working
        .keys()
        .find(|path| path.starts_with(&format!("{STATE}scopes/default/requirements/")))
        .expect("a Requirement file in default");
    let text = String::from_utf8(working[path].clone()).unwrap();
    (0..LONG)
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
            let parent = (index > 0).then(|| long_name(index - 1));
            let parents: Vec<&str> = parent.iter().map(String::as_str).collect();
            Commit::new(
                &long_name(index),
                &parents,
                "Reviewer",
                1_700_000_000 + 60 * index,
            )
            .with_file(path, &file)
        })
        .collect()
}

#[tokio::test]
async fn the_history_operations_at_the_edges_give_the_checked_answers() {
    let history = fixture();
    let missing = || {
        Some(Arc::new(Missing {
            held: MemoryFileVersions::new(&history.commits),
            missing: "alice",
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
