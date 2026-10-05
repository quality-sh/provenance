//! Record history and change evidence read from Git and the working copy.

mod evidence;
mod git;

pub(super) use evidence::page as evidence_page;

use super::classifier;
use crate::{cache::review_families, shards, state_store::StateStore};
use provenance_core::review::{RecordVersion, SaveOutcome};
use provenance_core::threads::{Discussion, DiscussionOrigin};
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
        let mut versions = Vec::new();
        let mut head = None;
        for file in git::file_versions(self.layout.root(), &path)? {
            head = file
                .text
                .as_deref()
                .map(|text| find(text, id))
                .transpose()?
                .flatten();
            if let Some(record) = &head {
                let source = Source {
                    id: StableId::new(file.commit.clone())?,
                    commit: Some(file.commit),
                    author: Some(file.author),
                    committed_at: Some(file.committed_at),
                };
                push_version(&mut versions, kind, source, record.clone())?;
            }
        }
        let working = working
            .as_deref()
            .map(|text| find(text, id))
            .transpose()?
            .flatten();
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
        for versioned in &mut versions {
            versioned.version.origin = origin(&discussions, kind, id, &versioned.version.revision);
        }
        Ok(versions)
    }
}

/// The record with `id` in the text of one JSONL record file.
fn find(text: &str, id: &StableId) -> anyhow::Result<Option<Value>> {
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let value: Value = serde_json::from_str(line)?;
        if value.get("id").and_then(Value::as_str) == Some(id.as_str()) {
            return Ok(Some(value));
        }
    }
    Ok(None)
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

/// The Discussion outcome that gave the record this revision.
fn origin(
    discussions: &[Discussion],
    kind: NodeType,
    id: &StableId,
    revision: &StableId,
) -> Option<DiscussionOrigin> {
    discussions.iter().find_map(|discussion| {
        discussion
            .outcomes
            .iter()
            .find(|outcome| {
                outcome.record_kind == kind
                    && outcome.record_id == *id
                    && outcome.revision == *revision
            })
            .map(|outcome| DiscussionOrigin {
                thread_id: discussion.thread_id.clone(),
                discussion_id: discussion.discussion_id.clone(),
                message_id: outcome.message_id.clone(),
            })
    })
}
