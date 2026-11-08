//! Archive commits that the history of `HEAD` does not contain.
//!
//! A squash or rebase merge drops the branch commit that an archive names.
//! The check asks the repository history once for each distinct commit. It
//! does not run with no history, without `HEAD`, or in a shallow clone,
//! because there the history is not known. [`GitHistory`] is the history of
//! a git work tree.

use super::StateStore;
use crate::repository::RepositoryHistory;
#[cfg(feature = "host")]
use crate::stale::git;
#[cfg(feature = "host")]
use camino::{Utf8Path, Utf8PathBuf};
use provenance_core::{ArchivedStamp, Manifest};
use provenance_macros::rule;
use std::collections::btree_map::{BTreeMap, Entry};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveCommitGap {
    pub scope_id: String,
    pub record_id: String,
    pub commit: String,
    pub reason: ArchiveCommitGapReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveCommitGapReason {
    Missing,
    NotAncestorOfHead,
}

impl fmt::Display for ArchiveCommitGap {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let reason = match self.reason {
            ArchiveCommitGapReason::Missing => "is not in this repository",
            ArchiveCommitGapReason::NotAncestorOfHead => "is not in the history of HEAD",
        };
        write!(
            formatter,
            "{}: archive commit {} {reason}",
            self.record_id, self.commit
        )
    }
}

impl StateStore {
    /// [`Self::archive_commits_outside`] with the git history of `repo`.
    #[cfg(feature = "host")]
    pub fn archive_commits_outside_history(
        &self,
        repo: &Utf8Path,
        manifest: &Manifest,
    ) -> anyhow::Result<Vec<ArchiveCommitGap>> {
        self.archive_commits_outside(&GitHistory::new(repo.to_owned()), manifest)
    }

    /// Each archived record in the scopes of `manifest` whose archive commit
    /// is missing from `history` or is not an ancestor of `HEAD`, in scope
    /// and record order.
    #[rule("rule_archive_commit_in_head_history")]
    pub fn archive_commits_outside(
        &self,
        history: &dyn RepositoryHistory,
        manifest: &Manifest,
    ) -> anyhow::Result<Vec<ArchiveCommitGap>> {
        let Ok(Some(head)) = history.complete_head() else {
            return Ok(Vec::new());
        };
        let mut stamps = Vec::new();
        for scope in &manifest.scopes {
            let scope = &scope.id;
            let mut push = |id: &str, stamp: &Option<ArchivedStamp>| {
                if let Some(stamp) = stamp {
                    stamps.push((
                        scope.as_str().to_string(),
                        id.to_string(),
                        stamp.commit.clone(),
                    ));
                }
            };
            for record in self.list_requirements(scope)? {
                push(record.id.as_str(), &record.archived_in_commit);
            }
            for record in self.list_resolutions(scope)? {
                push(record.id.as_str(), &record.archived_in_commit);
            }
            for record in self.list_rules(scope)? {
                push(record.id.as_str(), &record.archived_in_commit);
            }
            for record in self.list_topics(scope)? {
                push(record.id.as_str(), &record.archived_in_commit);
            }
            for record in self.list_questions(scope)? {
                push(record.id.as_str(), &record.archived_in_commit);
            }
        }
        let mut reasons = BTreeMap::new();
        let mut gaps = Vec::new();
        for (scope_id, record_id, commit) in stamps {
            let reason = match reasons.entry(commit.clone()) {
                Entry::Occupied(entry) => *entry.get(),
                Entry::Vacant(entry) => *entry.insert(commit_gap(history, &commit, &head)?),
            };
            if let Some(reason) = reason {
                gaps.push(ArchiveCommitGap {
                    scope_id,
                    record_id,
                    commit,
                    reason,
                });
            }
        }
        Ok(gaps)
    }
}

fn commit_gap(
    history: &dyn RepositoryHistory,
    commit: &str,
    head: &str,
) -> anyhow::Result<Option<ArchiveCommitGapReason>> {
    if !history.commit_exists(commit)? {
        return Ok(Some(ArchiveCommitGapReason::Missing));
    }
    Ok((!history.is_ancestor(commit, head)?).then_some(ArchiveCommitGapReason::NotAncestorOfHead))
}

/// The history of the git work tree at a root.
#[cfg(feature = "host")]
#[derive(Debug)]
pub struct GitHistory {
    root: Utf8PathBuf,
}

#[cfg(feature = "host")]
impl GitHistory {
    /// The history of the git work tree that holds `root`.
    pub const fn new(root: Utf8PathBuf) -> Self {
        Self { root }
    }

    /// The trimmed output of a git command, or `None` when it fails.
    fn probe(&self, args: &[&str]) -> Option<String> {
        git::command(&self.root)
            .args(args)
            .output()
            .ok()
            .filter(|output| output.status.success())
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .map(|text| text.trim().to_string())
    }
}

#[cfg(feature = "host")]
impl RepositoryHistory for GitHistory {
    /// A git that does not run, or a work tree with no commit, has no `HEAD`.
    fn head(&self) -> anyhow::Result<Option<String>> {
        let Ok(output) = crate::git_process::git()
            .args(["rev-parse", "HEAD"])
            .current_dir(&self.root)
            .output()
        else {
            return Ok(None);
        };
        if !output.status.success() {
            return Ok(None);
        }
        Ok(Some(String::from_utf8(output.stdout)?.trim().to_owned()))
    }

    fn complete_head(&self) -> anyhow::Result<Option<String>> {
        if self
            .probe(&["rev-parse", "--is-shallow-repository"])
            .as_deref()
            != Some("false")
        {
            return Ok(None);
        }
        Ok(self.probe(&["rev-parse", "--verify", "--quiet", "HEAD^{commit}"]))
    }

    fn commit_exists(&self, commit: &str) -> anyhow::Result<bool> {
        git::commit_exists(&self.root, commit)
    }

    fn is_ancestor(&self, commit: &str, head: &str) -> anyhow::Result<bool> {
        git::is_ancestor(&self.root, commit, head)
    }
}
