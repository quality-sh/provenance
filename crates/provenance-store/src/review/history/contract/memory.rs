//! The memory adapter of the file versions port, over a commit graph.

use super::{Adapter, Commit};
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
    files: BTreeMap<String, String>,
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
            for (path, text) in &commit.changes {
                match text {
                    Some(text) => files.insert(path.clone(), text.clone()),
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

    fn text(&self, index: usize, path: &str) -> Option<&str> {
        self.commits[index].files.get(path).map(String::as_str)
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
                FileVersion::new(
                    commit.name.clone(),
                    listed[&index]
                        .iter()
                        .map(|parent| self.commits[*parent].commit.name.clone())
                        .collect(),
                    commit.author.clone(),
                    commit.committed_at,
                    self.text(index, path).map(str::to_owned),
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
    Adapter::new(
        Arc::new(MemoryFileVersions::new(commits)),
        commits
            .iter()
            .map(|commit| (commit.name.clone(), commit.name.clone()))
            .collect(),
    )
}
