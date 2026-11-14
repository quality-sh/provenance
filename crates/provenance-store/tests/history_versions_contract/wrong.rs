//! Adapters that the contract must refuse, each wrong in one way, with the
//! commit names as commit ids.

use provenance_store::review::history::contract::{
    Adapter, Commit, Entry, Harness, MemoryFileVersions, MOVED_FROM, MOVED_TO,
};
use provenance_store::review::history::{BoxFuture, FileVersion, FileVersions};
use std::collections::BTreeMap;
use std::sync::Arc;

/// A harness over one wrong adapter.
pub struct Wrong(pub fn(&[Commit]) -> Arc<dyn FileVersions>);

impl Harness for Wrong {
    fn adapter(&self, commits: &[Commit]) -> Adapter {
        let ids = commits
            .iter()
            .map(|commit| (commit.name.clone(), commit.name.clone()))
            .collect();
        Adapter::new((self.0)(commits), ids)
    }
}

/// An adapter that compares a merge with its first parent only: it lists a
/// merge that keeps the file of its second parent.
pub fn first_parent_only(commits: &[Commit]) -> Arc<dyn FileVersions> {
    Arc::new(Walk::new(commits, Kept::FirstParentOnly))
}

/// An adapter that goes on through the last parent with the same file of a
/// merge, not the first.
pub fn last_same_parent(commits: &[Commit]) -> Arc<dyn FileVersions> {
    Arc::new(Walk::new(commits, Kept::LastParent))
}

/// An adapter that compares a merge with its first two parents only: it
/// lists an octopus merge that keeps the file of its third parent.
pub fn first_two_parents(commits: &[Commit]) -> Arc<dyn FileVersions> {
    Arc::new(Walk::new(commits, Kept::FirstTwoParents))
}

/// An adapter that lists a commit once for each parent line that reaches it.
pub fn each_line(commits: &[Commit]) -> Arc<dyn FileVersions> {
    Arc::new(Walk::new(commits, Kept::EachLine))
}

/// An adapter that compares the texts of a file and not its mode: it does
/// not list a commit that changes only the mode.
pub fn ignores_mode(commits: &[Commit]) -> Arc<dyn FileVersions> {
    Arc::new(MemoryFileVersions::new(&changed(commits, |entry| {
        Entry::new(entry.bytes, false)
    })))
}

/// An adapter that gives the text of `HEAD` at a commit that it does not
/// hold.
pub fn head_text(commits: &[Commit]) -> Arc<dyn FileVersions> {
    Arc::new(HeadText {
        memory: MemoryFileVersions::new(commits),
        commits: commits.iter().map(|commit| commit.name.clone()).collect(),
    })
}

/// An adapter that gives a text that is not UTF-8 with replacement
/// characters.
pub fn lossy_text(commits: &[Commit]) -> Arc<dyn FileVersions> {
    Arc::new(MemoryFileVersions::new(&changed(commits, |entry| {
        let text = String::from_utf8_lossy(&entry.bytes).into_owned();
        Entry::new(text.into_bytes(), entry.executable)
    })))
}

/// An adapter that lists the versions and gives an error for each text,
/// which is not the error of a text that is not UTF-8.
pub fn failed_text(commits: &[Commit]) -> Arc<dyn FileVersions> {
    Arc::new(FailedText(MemoryFileVersions::new(commits)))
}

/// `commits` with `change` applied to each entry.
fn changed(commits: &[Commit], change: fn(Entry) -> Entry) -> Vec<Commit> {
    let mut commits = commits.to_vec();
    for commit in &mut commits {
        for (_, entry) in &mut commit.changes {
            *entry = entry.take().map(change);
        }
    }
    commits
}

/// An adapter that gives each version the author time of its commit in
/// place of the commit time.
pub fn author_time(commits: &[Commit]) -> Arc<dyn FileVersions> {
    Arc::new(Other::new(commits, Reported::AuthorTime))
}

/// An adapter that gives each version the committer name of its commit in
/// place of the author name.
pub fn committer_name(commits: &[Commit]) -> Arc<dyn FileVersions> {
    Arc::new(Other::new(commits, Reported::Committer))
}

/// An adapter that follows the move of `MOVED_FROM` to `MOVED_TO`: the
/// versions of `MOVED_TO` start with the versions of `MOVED_FROM`.
pub fn follows_moves(commits: &[Commit]) -> Arc<dyn FileVersions> {
    Arc::new(FollowsMoves(MemoryFileVersions::new(commits)))
}

#[derive(Debug, Clone, Copy)]
enum Kept {
    FirstParentOnly,
    LastParent,
    FirstTwoParents,
    /// The first parent with the same file, with no check that a commit is
    /// listed already.
    EachLine,
}

/// The versions found from `HEAD` through the parent that `kept` names.
#[derive(Debug)]
struct Walk {
    commits: Vec<Commit>,
    files: Vec<BTreeMap<String, Entry>>,
    kept: Kept,
    memory: MemoryFileVersions,
}

impl Walk {
    fn new(commits: &[Commit], kept: Kept) -> Self {
        let mut files: Vec<BTreeMap<String, Entry>> = Vec::new();
        for commit in commits {
            let mut file = commit.parents.first().map_or_else(BTreeMap::new, |first| {
                files[Self::at(commits, first)].clone()
            });
            for (path, entry) in &commit.changes {
                match entry {
                    Some(entry) => file.insert(path.clone(), entry.clone()),
                    None => file.remove(path),
                };
            }
            files.push(file);
        }
        Self {
            commits: commits.to_vec(),
            files,
            kept,
            memory: MemoryFileVersions::new(commits),
        }
    }

    fn at(commits: &[Commit], name: &str) -> usize {
        commits
            .iter()
            .position(|commit| commit.name == name)
            .unwrap()
    }

    /// The nearest listed commits at `index`, which `listed` gets.
    fn nearest(
        &self,
        index: usize,
        path: &str,
        listed: &mut Vec<(usize, Vec<usize>)>,
    ) -> Vec<usize> {
        let parents = self.commits[index]
            .parents
            .iter()
            .map(|parent| Self::at(&self.commits, parent))
            .collect::<Vec<_>>();
        let text = self.files[index].get(path);
        if parents.is_empty() && text.is_none() {
            return Vec::new();
        }
        let same = |parent: &&usize| self.files[**parent].get(path) == text;
        let kept = match self.kept {
            Kept::FirstParentOnly => parents.first().filter(same),
            Kept::LastParent => parents.iter().rev().find(same),
            Kept::FirstTwoParents => parents.iter().take(2).find(same),
            Kept::EachLine => parents.iter().find(same),
        };
        if let Some(kept) = kept {
            return self.nearest(*kept, path, listed);
        }
        let each_line = matches!(self.kept, Kept::EachLine);
        if each_line || !listed.iter().any(|(known, _)| *known == index) {
            let lines = parents
                .iter()
                .flat_map(|parent| self.nearest(*parent, path, listed))
                .collect();
            listed.push((index, lines));
        }
        vec![index]
    }
}

impl FileVersions for Walk {
    fn versions<'a>(
        &'a self,
        path: &'a str,
    ) -> BoxFuture<'a, anyhow::Result<Arc<Vec<FileVersion>>>> {
        let mut listed = Vec::new();
        self.nearest(self.commits.len() - 1, path, &mut listed);
        // Each commit is listed after the commits it reaches.
        let versions = listed
            .into_iter()
            .map(|(index, parents)| {
                let commit = &self.commits[index];
                FileVersion::new(
                    commit.name.clone(),
                    parents
                        .iter()
                        .map(|parent| self.commits[*parent].name.clone())
                        .collect(),
                    commit.author.clone(),
                    commit.committed_at,
                )
            })
            .collect();
        Box::pin(std::future::ready(Ok(Arc::new(versions))))
    }

    fn text<'a>(
        &'a self,
        path: &'a str,
        commit: &'a str,
    ) -> BoxFuture<'a, anyhow::Result<Option<String>>> {
        self.memory.text(path, commit)
    }
}

#[derive(Debug, Clone, Copy)]
enum Reported {
    AuthorTime,
    Committer,
}

/// The memory adapter's versions with another field of each commit.
#[derive(Debug)]
struct Other {
    memory: MemoryFileVersions,
    commits: Vec<Commit>,
    reported: Reported,
}

impl Other {
    fn new(commits: &[Commit], reported: Reported) -> Self {
        Self {
            memory: MemoryFileVersions::new(commits),
            commits: commits.to_vec(),
            reported,
        }
    }

    fn changed(&self, version: &FileVersion) -> FileVersion {
        let commit = self
            .commits
            .iter()
            .find(|commit| commit.name == version.commit)
            .unwrap();
        let (author, time) = match self.reported {
            Reported::AuthorTime => (version.author.clone(), commit.authored_at),
            Reported::Committer => (commit.committer.clone(), version.committed_at),
        };
        FileVersion::new(
            version.commit.clone(),
            version.parents.clone(),
            author,
            time,
        )
    }
}

impl FileVersions for Other {
    fn versions<'a>(
        &'a self,
        path: &'a str,
    ) -> BoxFuture<'a, anyhow::Result<Arc<Vec<FileVersion>>>> {
        Box::pin(async move {
            let versions = self.memory.versions(path).await?;
            Ok(Arc::new(
                versions
                    .iter()
                    .map(|version| self.changed(version))
                    .collect(),
            ))
        })
    }

    fn text<'a>(
        &'a self,
        path: &'a str,
        commit: &'a str,
    ) -> BoxFuture<'a, anyhow::Result<Option<String>>> {
        self.memory.text(path, commit)
    }
}

/// The memory adapter, except that the versions of `MOVED_TO` go on
/// through the versions of `MOVED_FROM` before the move.
#[derive(Debug)]
struct FollowsMoves(MemoryFileVersions);

impl FileVersions for FollowsMoves {
    fn versions<'a>(
        &'a self,
        path: &'a str,
    ) -> BoxFuture<'a, anyhow::Result<Arc<Vec<FileVersion>>>> {
        Box::pin(async move {
            let versions = self.0.versions(path).await?;
            if path != MOVED_TO {
                return Ok(versions);
            }
            let mut earlier = self.0.versions(MOVED_FROM).await?.to_vec();
            // The last version of the old path is the move, which removed it.
            earlier.pop();
            let before = earlier.last().map(|version| version.commit.clone());
            for version in versions.iter() {
                let parents = if version.parents.is_empty() {
                    before.clone().into_iter().collect()
                } else {
                    version.parents.clone()
                };
                earlier.push(FileVersion::new(
                    version.commit.clone(),
                    parents,
                    version.author.clone(),
                    version.committed_at,
                ));
            }
            Ok(Arc::new(earlier))
        })
    }

    fn text<'a>(
        &'a self,
        path: &'a str,
        commit: &'a str,
    ) -> BoxFuture<'a, anyhow::Result<Option<String>>> {
        self.0.text(path, commit)
    }
}

/// The memory adapter, except that a commit it does not hold has the text
/// of `HEAD`.
#[derive(Debug)]
struct HeadText {
    memory: MemoryFileVersions,
    commits: Vec<String>,
}

impl FileVersions for HeadText {
    fn versions<'a>(
        &'a self,
        path: &'a str,
    ) -> BoxFuture<'a, anyhow::Result<Arc<Vec<FileVersion>>>> {
        self.memory.versions(path)
    }

    fn text<'a>(
        &'a self,
        path: &'a str,
        commit: &'a str,
    ) -> BoxFuture<'a, anyhow::Result<Option<String>>> {
        let known_commit = self.commits.iter().any(|known| known == commit);
        let head = self.commits.last().map_or("", String::as_str);
        self.memory
            .text(path, if known_commit { commit } else { head })
    }
}

/// The memory adapter, except that each text read fails.
#[derive(Debug)]
struct FailedText(MemoryFileVersions);

impl FileVersions for FailedText {
    fn versions<'a>(
        &'a self,
        path: &'a str,
    ) -> BoxFuture<'a, anyhow::Result<Arc<Vec<FileVersion>>>> {
        self.0.versions(path)
    }

    fn text<'a>(
        &'a self,
        _path: &'a str,
        _commit: &'a str,
    ) -> BoxFuture<'a, anyhow::Result<Option<String>>> {
        Box::pin(std::future::ready(Err(anyhow::anyhow!(
            "the history is not available"
        ))))
    }
}
