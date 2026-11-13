//! The contract that every file versions adapter meets.
//!
//! Each case asks the harness for an adapter over the commit graph
//! [`graph`]. The expected answers are the answers of the git adapter over a
//! Git history that holds the same commits, so an adapter that passes gives
//! the git adapter's versions and texts. The cases use only the port.

mod memory;

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

/// One commit of a commit graph.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Commit {
    pub name: String,
    /// The names of the parents, in parent order.
    pub parents: Vec<String>,
    /// The author name.
    pub author: String,
    /// The committer name.
    pub committer: String,
    /// The author time in seconds since the Unix epoch.
    pub authored_at: i64,
    /// The commit time in seconds since the Unix epoch.
    pub committed_at: i64,
    /// The files that differ from the first parent: the new text, or `None`
    /// for a removed file. A merge has the files of its first parent and
    /// these changes.
    pub changes: Vec<(String, Option<String>)>,
}

impl Commit {
    /// A commit with these parents, in parent order, that changes no file.
    /// Its author is also its committer, and its author time is its commit
    /// time.
    pub fn new(name: &str, parents: &[&str], author: &str, committed_at: i64) -> Self {
        Self {
            name: name.to_owned(),
            parents: parents.iter().map(|parent| (*parent).to_owned()).collect(),
            author: author.to_owned(),
            committer: author.to_owned(),
            authored_at: committed_at,
            committed_at,
            changes: Vec::new(),
        }
    }

    /// This commit with `committer` as the committer name and `authored_at`
    /// as the author time, as a commit that another person rebased has.
    #[must_use]
    pub fn with_committer(mut self, committer: &str, authored_at: i64) -> Self {
        committer.clone_into(&mut self.committer);
        self.authored_at = authored_at;
        self
    }

    /// This commit with `text` as the file at `path`.
    #[must_use]
    pub fn with_file(mut self, path: &str, text: &str) -> Self {
        self.changes.push((path.to_owned(), Some(text.to_owned())));
        self
    }

    /// This commit with no file at `path`.
    #[must_use]
    pub fn without_file(mut self, path: &str) -> Self {
        self.changes.push((path.to_owned(), None));
        self
    }
}

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
    fn ids(&self, names: &[&str]) -> Vec<String> {
        names.iter().map(|name| self.ids[*name].clone()).collect()
    }

    fn name(&self, id: &str) -> String {
        let Some((name, _)) = self.ids.iter().find(|(_, known)| known.as_str() == id) else {
            panic!("the adapter gave an unknown commit {id}")
        };
        name.clone()
    }

    /// The versions of `path` by commit name, after a check that each
    /// parent comes before its child.
    async fn versions(&self, path: &str) -> Vec<Named> {
        let versions = self.versions.versions(path).await.unwrap();
        let mut named: Vec<Named> = Vec::new();
        for version in versions.iter() {
            let commit = self.name(&version.commit);
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
            named.push((
                commit,
                parents,
                version.author.clone(),
                version.committed_at,
                version.text.clone(),
            ));
        }
        named
    }
}

fn named(commit: &str, parents: &[&str], text: Option<&str>) -> Named {
    let found = graph()
        .into_iter()
        .find(|known| known.name == commit)
        .expect("the case names a commit of the graph");
    (
        found.name,
        parents.iter().map(|parent| (*parent).to_owned()).collect(),
        found.author,
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

/// The versions of a file list each commit in the history of `HEAD` that
/// changed it, with the nearest listed commit on each parent line, each
/// parent before its child.
///
/// A merge that keeps the file of one parent, first or second, is not
/// listed, and the commits that only its other parents reach are not
/// listed. A merge with new text is listed with each parent line. A
/// removal is listed with no text. When more than one parent of a merge has
/// its file, the history goes on through the first of them. A moved file
/// starts at the move. The last version is the first commit that changed the
/// file on the walk from `HEAD`, which need not be `HEAD`. Each version
/// carries the author name and the commit time of its commit.
pub async fn versions_follow_the_history_of_head<H: Harness + Sync>(harness: &H) {
    let adapter = harness.adapter(&graph());
    let records = adapter.versions(RECORDS).await;
    assert_eq!(
        records.last().map(|version| version.0.as_str()),
        Some("revert"),
        "{RECORDS}: the last version is not the first commit that changed the file on the walk from HEAD"
    );
    assert_versions(
        RECORDS,
        records,
        vec![
            named("create", &[], Some("r1\n")),
            named("left", &["create"], Some("r2\n")),
            named("right", &["create"], Some("r3\n")),
            named("merge", &["left", "right"], Some("r4\n")),
            named("delete", &["merge"], None),
            named("again", &["delete"], Some("r9\n")),
            named("drift", &["again"], Some("r10\n")),
            named("revert", &["drift"], Some("r9\n")),
        ],
    );
    let discussions = adapter.versions(DISCUSSIONS).await;
    assert_eq!(
        discussions,
        vec![
            named("start", &[], Some("d0\n")),
            named("left", &["start"], Some("d2\n")),
            named("discuss", &["left"], Some("d5\n")),
        ]
    );
    assert_versions(
        MERGED,
        adapter.versions(MERGED).await,
        vec![
            named("split", &[], Some("m1\n")),
            named("pick", &["split"], Some("m2\n")),
        ],
    );
    assert_versions(
        MOVED_FROM,
        adapter.versions(MOVED_FROM).await,
        vec![
            named("draft", &[], Some("v1\n")),
            named("edit", &["draft"], Some("v2\n")),
            named("move", &["edit"], None),
        ],
    );
    assert_versions(
        MOVED_TO,
        adapter.versions(MOVED_TO).await,
        vec![named("move", &[], Some("v2\n"))],
    );
}

/// The texts of a file are its text at each named commit, in the order of
/// the names, whether or not that commit changed it: `None` before the file
/// exists and after its removal.
pub async fn texts_give_the_file_at_each_commit<H: Harness + Sync>(harness: &H) {
    let adapter = harness.adapter(&graph());
    let commits = adapter.ids(&[
        "again", "start", "discuss", "ours", "delete", "right", "theirs", "join", "other",
    ]);
    let texts = adapter.versions.texts(RECORDS, &commits).await.unwrap();
    let expected = [
        Some("r9\n"),
        None,
        Some("r4\n"),
        Some("r4\n"),
        None,
        Some("r3\n"),
        Some("r6\n"),
        Some("r9\n"),
        Some("r11\n"),
    ];
    assert_eq!(texts, expected.map(|text| text.map(str::to_owned)));
    let commits = adapter.ids(&["create", "right", "merge", "join"]);
    let texts = adapter.versions.texts(DISCUSSIONS, &commits).await.unwrap();
    let expected = [Some("d0\n"), Some("d0\n"), Some("d2\n"), Some("d5\n")];
    assert_eq!(texts, expected.map(|text| text.map(str::to_owned)));
}

/// A file that no commit holds has no versions and no text, and a read of
/// no commits gives no texts.
pub async fn a_file_that_no_commit_holds_has_no_versions<H: Harness + Sync>(harness: &H) {
    let adapter = harness.adapter(&graph());
    assert_eq!(adapter.versions(ABSENT).await, Vec::<Named>::new());
    let head = adapter.ids(&["move"]);
    assert_eq!(adapter.versions.texts(ABSENT, &head).await.unwrap(), [None]);
    let texts = adapter.versions.texts(RECORDS, &[]).await.unwrap();
    assert_eq!(texts, Vec::<Option<String>>::new());
}
