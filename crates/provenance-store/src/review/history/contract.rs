//! The contract that every file versions adapter meets.
//!
//! Each case asks the harness for an adapter over a commit graph, such as
//! [`graph`]. The expected answers are the answers of the git adapter over a
//! Git history that holds the same commits, so an adapter that passes gives
//! the git adapter's versions and texts. The cases use only the port.

mod cases;
mod commit;
mod edges;
mod memory;

pub use cases::{
    a_file_that_no_commit_holds_has_no_versions, text_gives_the_file_at_a_commit,
    versions_follow_the_history_of_head,
};
pub use commit::{Commit, Entry};
pub use edges::{
    a_change_of_mode_only_is_listed, a_commit_on_two_parent_lines_is_listed_once,
    a_commit_that_the_adapter_does_not_hold_has_no_text, a_text_that_is_not_utf8_is_an_error,
    an_octopus_merge_goes_on_through_the_parent_with_its_file, edge_graph, MODE, OCTOPUS,
    TWO_LINES, UNHELD, UNKNOWN_COMMIT, UNREADABLE,
};
pub use memory::{memory_adapter, MemoryFileVersions};

use super::versions::FileVersions;
use std::collections::BTreeMap;
use std::sync::Arc;

/// The record file of the graph.
pub const RECORDS: &str = ".provenance/state/records.jsonl";
/// The Discussion file of the graph.
pub const DISCUSSIONS: &str = ".provenance/state/discussions.jsonl";
/// A file that no commit of the graph holds.
pub const ABSENT: &str = ".provenance/state/absent.jsonl";
/// A file that two lines change to the same text before a merge.
pub const MERGED: &str = ".provenance/state/merged.jsonl";
/// A file that a commit moves to [`MOVED_TO`].
pub const MOVED_FROM: &str = ".provenance/state/moved_from.jsonl";
/// The file that a commit makes from [`MOVED_FROM`].
pub const MOVED_TO: &str = ".provenance/state/moved_to.jsonl";

/// The commit graph of the cases, each parent before its children. `HEAD`
/// is the last commit.
///
/// - `start` holds only the Discussion file.
/// - `left` and `right` change the record file on two lines from `create`;
///   `merge` gives it new text, so it is listed with both parents.
/// - `discuss` changes only the Discussion file.
/// - `ours` merges `theirs` and keeps the record file of its first parent
///   `discuss`, so neither is listed and `theirs` is not in the history of
///   the file.
/// - `delete` removes the record file and `again` adds it again.
/// - `drift` changes the record file and `revert` gives it the text of
///   `again` again; `other` changes it on a second line from `again`.
/// - `join` merges `other` and `revert` and keeps the record file of its
///   second parent `revert`, so neither `join` nor `other` is listed and
///   `revert` is the last version.
/// - `merge` and `revert` have a committer that is not their author and an
///   author time before their commit time.
/// - `pick` and `copy` give [`MERGED`] the same text on two lines from
///   `split`, and `same` merges them. The history goes on through the first
///   parent `pick`, so `copy` is not listed.
/// - `move` removes [`MOVED_FROM`] and adds [`MOVED_TO`] with the same text.
///   The versions do not follow the move: `move` is the first version of
///   [`MOVED_TO`], with no parent. `HEAD` is `move`.
#[must_use]
pub fn graph() -> Vec<Commit> {
    vec![
        commit("start", &[], "ann", 100).with_file(DISCUSSIONS, "d0\n"),
        commit("create", &["start"], "ann", 200).with_file(RECORDS, "r1\n"),
        commit("left", &["create"], "bea", 300)
            .with_file(RECORDS, "r2\n")
            .with_file(DISCUSSIONS, "d2\n"),
        commit("right", &["create"], "cal", 400).with_file(RECORDS, "r3\n"),
        commit("merge", &["left", "right"], "dan", 500)
            .with_committer("eve", 1_700_000_450)
            .with_file(RECORDS, "r4\n"),
        commit("discuss", &["merge"], "ann", 600).with_file(DISCUSSIONS, "d5\n"),
        commit("theirs", &["discuss"], "bea", 700).with_file(RECORDS, "r6\n"),
        commit("ours", &["discuss", "theirs"], "cal", 800),
        commit("delete", &["ours"], "dan", 900).without_file(RECORDS),
        commit("again", &["delete"], "ann", 1000).with_file(RECORDS, "r9\n"),
        commit("drift", &["again"], "bea", 1100).with_file(RECORDS, "r10\n"),
        commit("revert", &["drift"], "cal", 1200)
            .with_committer("eve", 1_700_001_150)
            .with_file(RECORDS, "r9\n"),
        commit("other", &["again"], "dan", 1300).with_file(RECORDS, "r11\n"),
        commit("join", &["other", "revert"], "ann", 1400).with_file(RECORDS, "r9\n"),
        commit("split", &["join"], "bea", 1500).with_file(MERGED, "m1\n"),
        commit("pick", &["split"], "cal", 1600).with_file(MERGED, "m2\n"),
        commit("copy", &["split"], "dan", 1700).with_file(MERGED, "m2\n"),
        commit("same", &["pick", "copy"], "ann", 1800),
        commit("draft", &["same"], "bea", 1900).with_file(MOVED_FROM, "v1\n"),
        commit("edit", &["draft"], "cal", 2000).with_file(MOVED_FROM, "v2\n"),
        commit("move", &["edit"], "dan", 2100)
            .without_file(MOVED_FROM)
            .with_file(MOVED_TO, "v2\n"),
    ]
}

fn commit(name: &str, parents: &[&str], author: &str, minute: i64) -> Commit {
    Commit::new(name, parents, author, 1_700_000_000 + minute)
}

/// An adapter over a history, and the commit id of each commit by name.
#[non_exhaustive]
pub struct Adapter {
    pub versions: Arc<dyn FileVersions>,
    pub ids: BTreeMap<String, String>,
}

impl Adapter {
    /// The adapter `versions`, whose commit ids by commit name are `ids`.
    pub const fn new(versions: Arc<dyn FileVersions>, ids: BTreeMap<String, String>) -> Self {
        Self { versions, ids }
    }
}

/// One adapter under test.
pub trait Harness {
    /// An adapter over a history that holds `commits`, with `HEAD` at the
    /// last commit.
    fn adapter(&self, commits: &[Commit]) -> Adapter;
}

/// One version as the cases compare it: the commit name, the parent
/// names, the author, the time and the text.
type Named = (String, Vec<String>, String, i64, Option<String>);

impl Adapter {
    fn name(&self, id: &str) -> String {
        let Some((name, _)) = self.ids.iter().find(|(_, known)| known.as_str() == id) else {
            panic!("the adapter gave an unknown commit {id}")
        };
        name.clone()
    }

    /// The text of `path` at the commit named `name`.
    async fn text(&self, path: &str, name: &str) -> Option<String> {
        self.versions.text(path, &self.ids[name]).await.unwrap()
    }

    /// The versions of `path` by commit name, each with its text, after a
    /// check that each parent comes before its child and that no commit is
    /// listed twice.
    async fn versions(&self, path: &str) -> Vec<Named> {
        let versions = self.versions.versions(path).await.unwrap();
        let mut named: Vec<Named> = Vec::new();
        for version in versions.iter() {
            let commit = self.name(&version.commit);
            assert!(
                named.iter().all(|listed| listed.0 != commit),
                "{path}: the commit {commit} is listed more than once"
            );
            let parents = version
                .parents
                .iter()
                .map(|parent| self.name(parent))
                .collect::<Vec<_>>();
            for parent in &parents {
                assert!(
                    named.iter().any(|listed| listed.0 == *parent),
                    "{path}: parent {parent} of {commit} is not listed before it"
                );
            }
            let text = self.text(path, &commit).await;
            named.push((
                commit,
                parents,
                version.author.clone(),
                version.committed_at,
                text,
            ));
        }
        named
    }
}

/// The expected version at the commit `commit` of `commits`.
fn named(commits: &[Commit], commit: &str, parents: &[&str], text: Option<&str>) -> Named {
    let found = commits
        .iter()
        .find(|known| known.name == commit)
        .expect("the case names a commit of the graph");
    (
        found.name.clone(),
        parents.iter().map(|parent| (*parent).to_owned()).collect(),
        found.author.clone(),
        found.committed_at,
        text.map(str::to_owned),
    )
}

/// Checks the versions of `path` against `expected`, in any parent-first
/// order: first the commits, parents and texts, then the author names, then
/// the commit times.
fn assert_versions(path: &str, mut versions: Vec<Named>, mut expected: Vec<Named>) {
    versions.sort();
    expected.sort();
    let commits = |list: &[Named]| {
        list.iter()
            .map(|version| (version.0.clone(), version.1.clone(), version.4.clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        commits(&versions),
        commits(&expected),
        "{path}: the listed commits, parents or texts are not the expected ones"
    );
    let authors = |list: &[Named]| {
        list.iter()
            .map(|version| version.2.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        authors(&versions),
        authors(&expected),
        "{path}: a version does not carry the author name of its commit"
    );
    let times = |list: &[Named]| list.iter().map(|version| version.3).collect::<Vec<_>>();
    assert_eq!(
        times(&versions),
        times(&expected),
        "{path}: a version does not carry the commit time of its commit"
    );
}
