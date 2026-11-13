//! The contract that every file versions adapter meets.
//!
//! Each case asks the harness for an adapter over the commit graph
//! [`GRAPH`]. The expected answers are the answers of the git adapter over a
//! Git history that holds the same commits, so an adapter that passes gives
//! the git adapter's versions and texts. The cases use only the port.

use super::versions::{BoxFuture, FileVersion, FileVersions};
use std::collections::BTreeMap;
use std::sync::Arc;

/// The record file of the graph.
pub const RECORDS: &str = ".provenance/state/records.jsonl";
/// The Discussion file of the graph.
pub const DISCUSSIONS: &str = ".provenance/state/discussions.jsonl";
/// A file that no commit of the graph holds.
pub const ABSENT: &str = ".provenance/state/absent.jsonl";

/// One commit of a commit graph.
#[derive(Debug, Clone, Copy)]
pub struct Commit {
    pub name: &'static str,
    /// The names of the parents, in parent order.
    pub parents: &'static [&'static str],
    pub author: &'static str,
    /// The author and commit time in seconds since the Unix epoch.
    pub committed_at: i64,
    /// The files that differ from the first parent: the new text, or `None`
    /// for a removed file. A merge has the files of its first parent and
    /// these changes.
    pub changes: &'static [(&'static str, Option<&'static str>)],
}

/// The commit graph of the cases, each parent before its children. `HEAD`
/// is the last commit.
///
/// - `start` holds only the Discussion file.
/// - `left` and `right` change the record file on two lines from `create`;
///   `merge` gives it new text, so it is listed with both parents.
/// - `discuss` changes only the Discussion file.
/// - `ours` merges `theirs` and keeps the record file of `discuss`, so
///   neither is listed and `theirs` is not in the history of the file.
/// - `delete` removes the record file and `again` adds it again.
pub const GRAPH: &[Commit] = &[
    commit("start", &[], "ann", 100, &[(DISCUSSIONS, Some("d0\n"))]),
    commit("create", &["start"], "ann", 200, &[(RECORDS, Some("r1\n"))]),
    commit(
        "left",
        &["create"],
        "bea",
        300,
        &[(RECORDS, Some("r2\n")), (DISCUSSIONS, Some("d2\n"))],
    ),
    commit("right", &["create"], "cal", 400, &[(RECORDS, Some("r3\n"))]),
    commit(
        "merge",
        &["left", "right"],
        "dan",
        500,
        &[(RECORDS, Some("r4\n"))],
    ),
    commit(
        "discuss",
        &["merge"],
        "ann",
        600,
        &[(DISCUSSIONS, Some("d5\n"))],
    ),
    commit(
        "theirs",
        &["discuss"],
        "bea",
        700,
        &[(RECORDS, Some("r6\n"))],
    ),
    commit("ours", &["discuss", "theirs"], "cal", 800, &[]),
    commit("delete", &["ours"], "dan", 900, &[(RECORDS, None)]),
    commit(
        "again",
        &["delete"],
        "ann",
        1000,
        &[(RECORDS, Some("r9\n"))],
    ),
];

const fn commit(
    name: &'static str,
    parents: &'static [&'static str],
    author: &'static str,
    minute: i64,
    changes: &'static [(&'static str, Option<&'static str>)],
) -> Commit {
    Commit {
        name,
        parents,
        author,
        committed_at: 1_700_000_000 + minute,
        changes,
    }
}

/// An adapter over a history, and the commit id of each commit by name.
pub struct Adapter {
    pub versions: Arc<dyn FileVersions>,
    pub ids: BTreeMap<&'static str, String>,
}

/// One adapter under test.
pub trait Harness {
    /// An adapter over a history that holds `commits`, with `HEAD` at the
    /// last commit.
    fn adapter(&self, commits: &[Commit]) -> Adapter;
}

/// One version as the cases compare it: the commit name, the parent
/// names, the author, the time and the text.
type Named = (&'static str, Vec<&'static str>, String, i64, Option<String>);

impl Adapter {
    fn ids(&self, names: &[&str]) -> Vec<String> {
        names.iter().map(|name| self.ids[name].clone()).collect()
    }

    fn name(&self, id: &str) -> &'static str {
        self.ids
            .iter()
            .find(|(_, known)| known.as_str() == id)
            .map(|(name, _)| *name)
            .unwrap_or_else(|| panic!("the adapter gave an unknown commit {id}"))
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

fn named(commit: &'static str, parents: &[&'static str], text: Option<&str>) -> Named {
    let found = GRAPH
        .iter()
        .find(|known| known.name == commit)
        .expect("the case names a commit of the graph");
    (
        commit,
        parents.to_vec(),
        found.author.to_owned(),
        found.committed_at,
        text.map(str::to_owned),
    )
}

/// Sorts the versions by commit name, so that two lists with the same
/// versions in another parent-first order compare equal.
fn sorted(mut versions: Vec<Named>) -> Vec<Named> {
    versions.sort_by_key(|version| version.0);
    versions
}

/// The versions of a file list each commit in the history of `HEAD` that
/// changed it, with the nearest listed commit on each parent line, each
/// parent before its child and the version of `HEAD` last.
///
/// A merge that keeps the file of one parent is not listed, and the commits
/// that only its other parents reach are not listed. A merge with new text
/// is listed with each parent line. A removal is listed with no text.
pub async fn versions_follow_the_history_of_head<H: Harness + Sync>(harness: &H) {
    let adapter = harness.adapter(GRAPH);
    let records = adapter.versions(RECORDS).await;
    assert_eq!(records.last().map(|version| version.0), Some("again"));
    assert_eq!(
        sorted(records),
        sorted(vec![
            named("create", &[], Some("r1\n")),
            named("left", &["create"], Some("r2\n")),
            named("right", &["create"], Some("r3\n")),
            named("merge", &["left", "right"], Some("r4\n")),
            named("delete", &["merge"], None),
            named("again", &["delete"], Some("r9\n")),
        ])
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
}

/// The texts of a file are its text at each named commit, in the order of
/// the names, whether or not that commit changed it: `None` before the file
/// exists and after its removal.
pub async fn texts_give_the_file_at_each_commit<H: Harness + Sync>(harness: &H) {
    let adapter = harness.adapter(GRAPH);
    let commits = adapter.ids(&[
        "again", "start", "discuss", "ours", "delete", "right", "theirs",
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
    ];
    assert_eq!(texts, expected.map(|text| text.map(str::to_owned)));
    let commits = adapter.ids(&["create", "right", "merge", "again"]);
    let texts = adapter.versions.texts(DISCUSSIONS, &commits).await.unwrap();
    let expected = [Some("d0\n"), Some("d0\n"), Some("d2\n"), Some("d5\n")];
    assert_eq!(texts, expected.map(|text| text.map(str::to_owned)));
}

/// A file that no commit holds has no versions and no text, and a read of
/// no commits gives no texts.
pub async fn a_file_that_no_commit_holds_has_no_versions<H: Harness + Sync>(harness: &H) {
    let adapter = harness.adapter(GRAPH);
    assert_eq!(adapter.versions(ABSENT).await, Vec::<Named>::new());
    let head = adapter.ids(&["again"]);
    assert_eq!(adapter.versions.texts(ABSENT, &head).await.unwrap(), [None]);
    let texts = adapter.versions.texts(RECORDS, &[]).await.unwrap();
    assert_eq!(texts, Vec::<Option<String>>::new());
}

/// File versions over a commit graph in memory, with the commit names as
/// commit ids. It lists versions as the git adapter does.
#[derive(Debug, Clone)]
pub struct MemoryFileVersions {
    commits: Vec<Stored>,
}

#[derive(Debug, Clone)]
struct Stored {
    commit: Commit,
    files: BTreeMap<&'static str, &'static str>,
}

impl MemoryFileVersions {
    /// The history of `commits`, each parent before its children, with
    /// `HEAD` at the last commit.
    ///
    /// # Panics
    ///
    /// When a commit names a parent that is not before it.
    pub fn new(commits: &[Commit]) -> Self {
        let mut stored: Vec<Stored> = Vec::new();
        for commit in commits {
            let mut files = commit.parents.first().map_or_else(BTreeMap::new, |first| {
                stored
                    .iter()
                    .find(|known| known.commit.name == *first)
                    .expect("a parent comes before its child")
                    .files
                    .clone()
            });
            for (path, text) in commit.changes {
                match text {
                    Some(text) => files.insert(path, text),
                    None => files.remove(path),
                };
            }
            stored.push(Stored {
                commit: *commit,
                files,
            });
        }
        Self { commits: stored }
    }

    fn index(&self, name: &str) -> Option<usize> {
        self.commits
            .iter()
            .position(|stored| stored.commit.name == name)
    }

    fn text(&self, index: usize, path: &str) -> Option<&'static str> {
        self.commits[index].files.get(path).copied()
    }

    /// The nearest listed commits at or below `index`. A commit is listed
    /// when its file differs from the file of each parent; a commit whose
    /// file is the file of a parent goes on through the first such parent
    /// only. `listed` gets each listed commit with its listed parents.
    fn nearest(
        &self,
        index: usize,
        path: &str,
        listed: &mut BTreeMap<usize, Vec<usize>>,
    ) -> Vec<usize> {
        let parents = self.commits[index]
            .commit
            .parents
            .iter()
            .filter_map(|parent| self.index(parent))
            .collect::<Vec<_>>();
        if parents.is_empty() && self.text(index, path).is_none() {
            return Vec::new();
        }
        if let Some(kept) = parents
            .iter()
            .find(|parent| self.text(**parent, path) == self.text(index, path))
        {
            return self.nearest(*kept, path, listed);
        }
        if !listed.contains_key(&index) {
            let mut lines = Vec::new();
            for parent in parents {
                for nearest in self.nearest(parent, path, listed) {
                    if !lines.contains(&nearest) {
                        lines.push(nearest);
                    }
                }
            }
            listed.insert(index, lines);
        }
        vec![index]
    }

    /// Adds `index` after its listed parents.
    fn order(index: usize, listed: &BTreeMap<usize, Vec<usize>>, order: &mut Vec<usize>) {
        if order.contains(&index) {
            return;
        }
        for parent in &listed[&index] {
            Self::order(*parent, listed, order);
        }
        order.push(index);
    }

    fn file_versions(&self, path: &str) -> Vec<FileVersion> {
        let Some(head) = self.commits.len().checked_sub(1) else {
            return Vec::new();
        };
        let mut listed = BTreeMap::new();
        let mut order = Vec::new();
        for tip in self.nearest(head, path, &mut listed) {
            Self::order(tip, &listed, &mut order);
        }
        order
            .into_iter()
            .map(|index| {
                let commit = &self.commits[index].commit;
                FileVersion {
                    commit: commit.name.to_owned(),
                    parents: listed[&index]
                        .iter()
                        .map(|parent| self.commits[*parent].commit.name.to_owned())
                        .collect(),
                    author: commit.author.to_owned(),
                    committed_at: commit.committed_at,
                    text: self.text(index, path).map(str::to_owned),
                }
            })
            .collect()
    }
}

impl FileVersions for MemoryFileVersions {
    fn versions<'a>(
        &'a self,
        path: &'a str,
    ) -> BoxFuture<'a, anyhow::Result<Arc<Vec<FileVersion>>>> {
        Box::pin(std::future::ready(Ok(Arc::new(self.file_versions(path)))))
    }

    fn texts<'a>(
        &'a self,
        path: &'a str,
        commits: &'a [String],
    ) -> BoxFuture<'a, anyhow::Result<Vec<Option<String>>>> {
        let texts = commits
            .iter()
            .map(|commit| {
                self.index(commit)
                    .and_then(|index| self.text(index, path))
                    .map(str::to_owned)
            })
            .collect();
        Box::pin(std::future::ready(Ok(texts)))
    }
}

/// The memory adapter over `commits`, for a harness.
#[must_use]
pub fn memory_adapter(commits: &[Commit]) -> Adapter {
    Adapter {
        versions: Arc::new(MemoryFileVersions::new(commits)),
        ids: commits
            .iter()
            .map(|commit| (commit.name, commit.name.to_owned()))
            .collect(),
    }
}
