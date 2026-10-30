//! Archive commits that the history of `HEAD` does not contain.
//!
//! A squash or rebase merge drops the branch commit that an archive names.
//! The check asks git once for each distinct commit. It does not run outside
//! a git work tree, without `HEAD`, or in a shallow clone, because there the
//! history is not known.

use super::StateStore;
use crate::stale::git;
use camino::Utf8Path;
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
    /// Each archived record in the scopes of `manifest` whose archive commit
    /// is missing or is not an ancestor of `HEAD`, in scope and record order.
    #[rule("rule_archive_commit_in_head_history")]
    pub fn archive_commits_outside_history(
        &self,
        repo: &Utf8Path,
        manifest: &Manifest,
    ) -> anyhow::Result<Vec<ArchiveCommitGap>> {
        let Some(head) = full_history_head(repo) else {
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
                Entry::Vacant(entry) => *entry.insert(commit_gap(repo, &commit, &head)?),
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
    repo: &Utf8Path,
    commit: &str,
    head: &str,
) -> anyhow::Result<Option<ArchiveCommitGapReason>> {
    if !git::commit_exists(repo, commit)? {
        return Ok(Some(ArchiveCommitGapReason::Missing));
    }
    Ok((!git::is_ancestor(repo, commit, head)?)
        .then_some(ArchiveCommitGapReason::NotAncestorOfHead))
}

/// The `HEAD` commit when the repository has its full history.
fn full_history_head(repo: &Utf8Path) -> Option<String> {
    let probe = |args: &[&str]| {
        git::command(repo)
            .args(args)
            .output()
            .ok()
            .filter(|output| output.status.success())
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .map(|text| text.trim().to_string())
    };
    if probe(&["rev-parse", "--is-shallow-repository"])? != "false" {
        return None;
    }
    probe(&["rev-parse", "--verify", "--quiet", "HEAD^{commit}"])
}
