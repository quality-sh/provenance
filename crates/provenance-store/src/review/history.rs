//! Record history and change evidence read from Git and the working copy.

mod evidence;
mod git;
mod origins;

pub(super) use evidence::page as evidence_page;

use super::classifier;
use crate::{cache::review_families, shards, state_store::StateStore};
use provenance_core::review::{RecordVersion, SaveOutcome};
use provenance_core::{NodeType, ScopeId, StableId};
use provenance_macros::rule;
use serde_json::Value;

/// The version id of the saved working copy.
pub(super) const WORKING: &str = "working";

/// One record version and the record JSON it holds.
pub(super) struct VersionedRecord {
    pub(super) version: RecordVersion,
    pub(super) record: Value,
}

/// Where one version comes from.
struct Source {
    id: StableId,
    commit: Option<String>,
    author: Option<String>,
    committed_at: Option<i64>,
}

impl StateStore {
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
    #[rule("rule_record_history_reads_git")]
    pub(super) fn record_versions(
        &self,
        scope: &ScopeId,
        kind: NodeType,
        id: &StableId,
    ) -> anyhow::Result<Vec<VersionedRecord>> {
        let path = shards::path_for(&self.layout, scope, kind);
        let (working, discussions) = self.with_repository_publication(|| {
            let text = match std::fs::read_to_string(&path) {
                Ok(text) => Some(text),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Err(error) => return Err(error.into()),
            };
            Ok((text, self.list_discussions(scope)?))
        })?;
        let files = git::file_versions(self.layout.root(), &path)?;
        let records = files
            .iter()
            .map(|file| file.text.as_deref().and_then(|text| find(text, id)))
            .collect::<Vec<_>>();
        let head = records.last().cloned().flatten();
        let mut versions = Vec::new();
        for index in lineage(&files, &records) {
            let (file, Some(record)) = (&files[index], records[index].clone()) else {
                continue;
            };
            let source = Source {
                id: StableId::new(file.commit.clone())?,
                commit: Some(file.commit.clone()),
                author: Some(file.author.clone()),
                committed_at: Some(file.committed_at),
            };
            push_version(&mut versions, kind, source, record)?;
        }
        let working = working.as_deref().and_then(|text| find(text, id));
        if let Some(record) = working {
            if head
                .as_ref()
                .is_none_or(|head| content(head) != content(&record))
            {
                let source = Source {
                    id: StableId::new(WORKING)?,
                    commit: None,
                    author: None,
                    committed_at: None,
                };
                push_version(&mut versions, kind, source, record)?;
            }
        }
        origins::assign(
            self.layout.root(),
            &shards::discussions_path(&self.layout, scope),
            &discussions,
            kind,
            id,
            &mut versions,
        )?;
        Ok(versions)
    }
}

/// The indexes of the commits that changed the record at `HEAD`, oldest
/// first. The walk starts at the newest commit and steps to a parent whose
/// record the commit kept, or else to its first parent. A commit that holds
/// the record and kept no parent's record changed it.
fn lineage(files: &[git::FileVersion], records: &[Option<Value>]) -> Vec<usize> {
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
        let record = records[current].as_ref().map(content);
        let kept = parents
            .iter()
            .copied()
            .find(|parent| records[*parent].as_ref().map(content) == record);
        if kept.is_none() && record.is_some() {
            chain.push(current);
        }
        at = kept.or_else(|| parents.first().copied());
    }
    chain.reverse();
    chain
}

/// The record with `id` in the text of one JSONL record file. A line that is
/// not a JSON record, such as a committed conflict marker, holds no record.
fn find(text: &str, id: &StableId) -> Option<Value> {
    text.lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .find(|value| value.get("id").and_then(Value::as_str) == Some(id.as_str()))
}

/// The record without its schema version and record stamps.
fn content(record: &Value) -> Value {
    let mut value = record.clone();
    if let Some(fields) = value.as_object_mut() {
        for name in ["schema_version", "created", "updated"] {
            fields.remove(name);
        }
    }
    value
}

/// Adds a version when the record differs from the previous version.
fn push_version(
    versions: &mut Vec<VersionedRecord>,
    kind: NodeType,
    source: Source,
    record: Value,
) -> anyhow::Result<()> {
    let after = content(&record);
    let previous = versions.last();
    let changed_fields = match previous.map(|previous| content(&previous.record)) {
        None => after
            .as_object()
            .map(|fields| fields.keys().cloned().collect())
            .unwrap_or_default(),
        Some(before) => {
            let mut names = before
                .as_object()
                .into_iter()
                .chain(after.as_object())
                .flat_map(|fields| fields.keys().cloned())
                .collect::<Vec<_>>();
            names.sort();
            names.dedup();
            names.retain(|name| before.get(name) != after.get(name));
            if names.is_empty() {
                return Ok(());
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
        revision: classifier::review_revision(kind, &record)?,
        before: previous.map(|previous| previous.version.id.clone()),
        changed_fields,
        outcome,
        origin: None,
    };
    versions.push(VersionedRecord { version, record });
    Ok(())
}
