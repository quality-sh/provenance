//! Record history and change evidence, read from the committed versions of
//! the record files and from the working copy.
//!
//! The committed versions come through the [`FileVersions`] port and the
//! working copy through the storage of the store, so the history reads
//! build without `host`. The git adapter of the port needs `host`.

pub mod contract;
mod evidence;
mod found;
#[cfg(feature = "host")]
mod git;
pub(super) mod index;
mod origins;
mod texts;
mod versions;

pub(super) use evidence::page as evidence_page;
#[cfg(feature = "host")]
pub use git::GitFileVersions;
pub(crate) use versions::NoFileVersions;
pub use versions::{BoxFuture, FileVersion, FileVersions};

use crate::{
    families::review_families,
    jsonl::{committed_line_value, numbered_lines},
    shards,
    state_store::StateStore,
};
use index::{Held, RecordIndex};
use provenance_core::protocol::read_failure::ReadFailure;
use provenance_core::review::{RecordVersion, SaveOutcome};
use provenance_core::{NodeType, ScopeId, StableId};
use provenance_macros::rule;
use serde_json::Value;
use std::collections::BTreeMap;

/// The version id of the saved working copy.
pub(super) const WORKING: &str = "working";

/// The most versions of a record file that one history read indexes.
const HISTORY_FILE_VERSIONS: usize = 1_000;

/// One record version and what the record holds at it. The saved working
/// copy also gives its record JSON; a read finds the record JSON of a
/// committed version in the text of its commit.
#[derive(Clone)]
pub(super) struct VersionedRecord {
    pub(super) version: RecordVersion,
    pub(super) held: Held,
    pub(super) record: Option<Value>,
}

/// The repository paths of the record file of one kind and of the
/// Discussion file of a scope, as the file versions port names them.
pub(super) struct HistoryPaths {
    pub(super) records: String,
    pub(super) discussions: String,
}

/// Where one version comes from.
struct Source {
    id: StableId,
    commit: Option<String>,
    author: Option<String>,
    committed_at: Option<i64>,
}

impl StateStore {
    /// The paths of the record file of `kind` and of the Discussion file in
    /// `scope`.
    pub(super) fn history_paths(
        &self,
        scope: &ScopeId,
        kind: NodeType,
    ) -> anyhow::Result<HistoryPaths> {
        Ok(HistoryPaths {
            records: self.relative(&shards::path_for(&self.layout, scope, kind))?,
            discussions: self.relative(&shards::discussions_path(&self.layout, scope))?,
        })
    }

    /// The listed versions of the file `path`.
    pub(super) async fn file_versions_of(
        &self,
        path: &str,
    ) -> anyhow::Result<std::sync::Arc<Vec<FileVersion>>> {
        self.file_versions().versions(path).await
    }

    /// The record `id` in the record file `path` at `commit`: one text.
    pub(super) async fn record_at(
        &self,
        path: &str,
        commit: &str,
        id: &StableId,
    ) -> anyhow::Result<Option<Value>> {
        let text = self.file_versions().text(path, commit).await?;
        Ok(text.as_deref().and_then(|text| find(text, id)))
    }

    /// Lists the versions of one record, oldest first: each commit that
    /// changed it in its graph record file, then the saved working copy when
    /// it differs from the record at `HEAD`.
    ///
    /// Each version follows the version of its own parent's record. When
    /// `HEAD` holds the record, the last commit version holds that record.
    /// A merge that keeps one side's record unchanged adds no version and
    /// lists only the kept side. A merge that resolves to new content adds
    /// one, after the version of its first parent. A commit whose shard text
    /// does not parse adds no version.
    ///
    /// The committed versions come from the listed versions `files` of the
    /// record file and from the index of that file. The rows that the read
    /// adds stay in `index`, also when the read fails.
    #[rule("rule_record_history_reads_git")]
    pub(super) async fn read_record_versions(
        &self,
        scope: &ScopeId,
        kind: NodeType,
        id: &StableId,
        files: &[FileVersion],
        index: &mut RecordIndex,
    ) -> anyhow::Result<Vec<VersionedRecord>> {
        let paths = self.history_paths(scope, kind)?;
        let path = shards::path_for(&self.layout, scope, kind);
        let (working, discussions) = self.with_repository_publication(|held| {
            Ok((held.state_text(&path)?, held.list_discussions(scope)?))
        })?;
        let history = self.file_versions();
        let found = found::Paths {
            records: &paths.records,
            discussions: &paths.discussions,
        };
        let committed = found::committed(history.as_ref(), files, found, kind, id, index).await?;
        let mut versions = committed.versions;
        let working = working.as_deref().and_then(|text| find(text, id));
        if let Some(record) = working {
            let held = Held::of(kind, &record)?;
            if committed
                .head
                .as_ref()
                .is_none_or(|head| head.digest != held.digest)
            {
                let source = Source {
                    id: StableId::new(WORKING)?,
                    commit: None,
                    author: None,
                    committed_at: None,
                };
                let count = versions.len();
                push_version(&mut versions, kind, source, held, Some(record));
                if let Some(version) = versions.get_mut(count) {
                    origins::working(discussions, kind, id, &committed.seen, version);
                }
            }
        }
        Ok(versions)
    }

    /// The path of a file of this store below its working tree, as the file
    /// versions port names it.
    fn relative(&self, path: &camino::Utf8Path) -> anyhow::Result<String> {
        Ok(path
            .strip_prefix(self.layout.root())?
            .as_str()
            .replace('\\', "/"))
    }
}

/// Refuses with the page budget refusal a read of a record file with more
/// versions to index than one history read indexes. The refusal says how
/// many versions of the file at `path` the read indexed and how many wait.
#[rule("rule_history_reads_bound_file_versions")]
fn bound_file_versions(path: &str, added: usize, waiting: usize) -> anyhow::Result<()> {
    if waiting > HISTORY_FILE_VERSIONS {
        return Err(
            anyhow::Error::from(ReadFailure::PageBudgetExceeded).context(Indexing {
                path: path.to_owned(),
                added,
                waiting,
            }),
        );
    }
    Ok(())
}

/// The cause of a page budget refusal of a history read that put versions
/// in the index while more wait: a repeat of the read continues.
#[derive(Debug)]
pub(super) struct Indexing {
    pub(super) path: String,
    pub(super) added: usize,
    pub(super) waiting: usize,
}

impl std::fmt::Display for Indexing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "the history read put {} versions of {} in the index of record changes; versions \
             that wait: {}; a repeat of the read continues",
            self.added,
            self.path,
            self.waiting - self.added,
        )
    }
}

/// The indexes of the commits that changed the record at `HEAD`, oldest
/// first. The walk starts at the newest commit and steps to a parent whose
/// record the commit kept, or else to its first parent. A commit that holds
/// the record and kept no parent's record changed it.
fn lineage(files: &[FileVersion], contents: &[Option<&str>]) -> Vec<usize> {
    let index = files
        .iter()
        .enumerate()
        .map(|(index, file)| (file.commit.as_str(), index))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut chain = Vec::new();
    let mut at = files.len().checked_sub(1);
    // A parent is listed before its child, so each step goes to a lower index.
    while let Some(current) = at {
        let parents = files[current]
            .parents
            .iter()
            .filter_map(|parent| index.get(parent.as_str()).copied())
            .collect::<Vec<_>>();
        let record = contents[current];
        let kept = parents
            .iter()
            .copied()
            .find(|parent| contents[*parent] == record);
        if kept.is_none() && record.is_some() {
            chain.push(current);
        }
        at = kept.or_else(|| parents.first().copied());
    }
    chain.reverse();
    chain
}

/// The record with `id` in the text of one JSONL record file. A line that
/// [`committed_line_value`] does not read, such as a committed conflict marker
/// or a line that holds a top-level member two times, holds no record.
fn find(text: &str, id: &StableId) -> Option<Value> {
    numbered_lines(text.as_bytes())
        .filter_map(|(_, line)| committed_line_value(line))
        .find(|value| value.get("id").and_then(Value::as_str) == Some(id.as_str()))
}

/// Adds a version when the record differs from the previous version, and
/// tells whether it did.
fn push_version(
    versions: &mut Vec<VersionedRecord>,
    kind: NodeType,
    source: Source,
    held: Held,
    record: Option<Value>,
) -> bool {
    let after = held.content().collect::<BTreeMap<_, _>>();
    let previous = versions.last();
    let changed_fields = match previous {
        None => after.keys().map(|name| (*name).clone()).collect(),
        Some(previous) => {
            let before = previous.held.content().collect::<BTreeMap<_, _>>();
            let mut names = before
                .keys()
                .chain(after.keys())
                .map(|name| (*name).clone())
                .collect::<Vec<_>>();
            names.sort();
            names.dedup();
            names.retain(|name| before.get(name) != after.get(name));
            if names.is_empty() {
                return false;
            }
            names
        }
    };
    let content_fields = review_families::by_kind(kind).content_fields;
    let outcome = if previous.is_none() {
        SaveOutcome::Created
    } else if changed_fields
        .iter()
        .any(|field| content_fields.contains(&field.as_str()))
    {
        SaveOutcome::Changed
    } else {
        SaveOutcome::LifecycleOnly
    };
    let version = RecordVersion {
        id: source.id,
        commit: source.commit,
        author: source.author,
        committed_at: source.committed_at,
        revision: held.revision.clone(),
        before: previous.map(|previous| previous.version.id.clone()),
        changed_fields,
        outcome,
        origin: None,
    };
    versions.push(VersionedRecord {
        version,
        held,
        record,
    });
    true
}
