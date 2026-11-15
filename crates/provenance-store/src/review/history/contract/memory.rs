//! The memory adapter of the file versions port, over a commit graph.

use super::{Adapter, Commit, Entry};
use crate::review::history::versions::{BoxFuture, FileVersion, FileVersions};
use std::collections::BTreeMap;
use std::sync::Arc;

/// File versions over a commit graph in memory, with the commit names as
/// commit ids. It lists versions as the git adapter does.
#[derive(Debug, Clone)]
pub struct MemoryFileVersions {
    commits: Vec<Stored>,
}

#[derive(Debug, Clone)]
struct Stored {
    commit: Commit,
    files: BTreeMap<String, Entry>,
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
            for (path, entry) in &commit.changes {
                match entry {
                    Some(entry) => files.insert(path.clone(), entry.clone()),
                    None => files.remove(path),
                };
            }
            stored.push(Stored {
                commit: commit.clone(),
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

    fn entry(&self, index: usize, path: &str) -> Option<&Entry> {
        self.commits[index].files.get(path)
    }

    /// The nearest listed commits at or below `index`. A commit is listed
    /// when its entry, with its mode, differs from the entry of each parent;
    /// a commit whose entry is the entry of a parent goes on through the
    /// first such parent only. `listed` gets each listed commit with its listed parents.
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
        if parents.is_empty() && self.entry(index, path).is_none() {
            return Vec::new();
        }
        if let Some(kept) = parents
            .iter()
            .find(|parent| self.entry(**parent, path) == self.entry(index, path))
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
                FileVersion::new(
                    commit.name.clone(),
                    listed[&index]
                        .iter()
                        .map(|parent| self.commits[*parent].commit.name.clone())
                        .collect(),
                    commit.author.clone(),
                    commit.committed_at,
                )
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

    fn text<'a>(
        &'a self,
        path: &'a str,
        commit: &'a str,
    ) -> BoxFuture<'a, anyhow::Result<Option<String>>> {
        let text = self
            .index(commit)
            .and_then(|index| self.entry(index, path))
            .map(|entry| String::from_utf8(entry.bytes.clone()))
            .transpose()
            .map_err(anyhow::Error::from);
        Box::pin(std::future::ready(text))
    }
}

/// The memory adapter over `commits`, for a harness.
#[must_use]
pub fn memory_adapter(commits: &[Commit]) -> Adapter {
    Adapter::new(
        Arc::new(MemoryFileVersions::new(commits)),
        commits
            .iter()
            .map(|commit| (commit.name.clone(), commit.name.clone()))
            .collect(),
    )
}
