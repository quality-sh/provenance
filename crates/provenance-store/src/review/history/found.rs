//! The committed versions of one record, from the index of its record file.
//! A read first indexes the listed commits that the index does not hold, at
//! most [`HISTORY_FILE_VERSIONS`](super::HISTORY_FILE_VERSIONS) of them.

use super::index::{Held, RecordIndex};
use super::origins::{self, Outcome};
use super::texts::each_text;
use super::versions::{FileVersion, FileVersions};
use super::{bound_file_versions, lineage, push_version, Source, VersionedRecord};
use crate::jsonl::{committed_line_value, numbered_lines};
use provenance_core::{NodeType, StableId};
use provenance_macros::rule;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// The committed versions of one record, each with its outcomes.
pub(super) struct Committed {
    pub(super) versions: Vec<VersionedRecord>,
    /// What the record holds at the last listed version of its file.
    pub(super) head: Option<Held>,
    /// The outcomes that the committed versions hold.
    pub(super) seen: BTreeSet<Outcome>,
}

/// The paths of the record file and of the Discussion file of a record.
pub(super) struct Paths<'a> {
    pub(super) records: &'a str,
    pub(super) discussions: &'a str,
}

/// The committed versions of the record `id` over the versions `files` of
/// its record file. The read first puts the commits that `index` does not
/// hold in it. When more commits wait than one read indexes, the read gives
/// the page budget refusal after it indexed as many as it may.
#[rule("rule_record_history_reads_index")]
pub(super) async fn committed(
    history: &dyn FileVersions,
    files: &[FileVersion],
    paths: Paths<'_>,
    kind: NodeType,
    id: &StableId,
    index: &mut RecordIndex,
) -> anyhow::Result<Committed> {
    let waiting = index.waiting(files);
    let count = waiting.len().min(super::HISTORY_FILE_VERSIONS);
    extend(history, &paths, kind, id, &waiting[..count], index).await?;
    bound_file_versions(index.path(), count, waiting.len())?;
    let states = index.states(files);
    let contents = states
        .iter()
        .map(|state| state.map(|held| held.digest.as_str()))
        .collect::<Vec<_>>();
    let mut versions = Vec::new();
    let mut outcomes = Vec::new();
    for at in lineage(files, &contents) {
        let (file, Some(held)) = (&files[at], states[at]) else {
            continue;
        };
        let source = Source {
            id: StableId::new(file.commit.clone())?,
            commit: Some(file.commit.clone()),
            author: Some(file.author.clone()),
            committed_at: Some(file.committed_at),
        };
        if push_version(&mut versions, kind, source, held.clone(), None) {
            outcomes.push(&held.outcomes);
        }
    }
    let mut seen = BTreeSet::new();
    for (version, outcomes) in versions.iter_mut().zip(outcomes) {
        origins::committed(outcomes, &mut seen, version);
    }
    Ok(Committed {
        versions,
        head: states.last().copied().flatten().cloned(),
        seen,
    })
}

/// The records of one record file text by id, each with its first line.
fn records(text: Option<&str>) -> BTreeMap<String, Value> {
    let mut records = BTreeMap::new();
    for (_, line) in numbered_lines(text.unwrap_or_default().as_bytes()) {
        let Some(value) = committed_line_value(line) else {
            continue;
        };
        if let Some(id) = value.get("id").and_then(Value::as_str) {
            records.entry(id.to_owned()).or_insert(value);
        }
    }
    records
}

/// Puts the commits `waiting` in `index`, in list order, each with its
/// base: one text of the record file for each, then one text of the
/// Discussion file for each that holds a changed record.
async fn extend(
    history: &dyn FileVersions,
    paths: &Paths<'_>,
    kind: NodeType,
    id: &StableId,
    waiting: &[(&FileVersion, Option<String>)],
    index: &mut RecordIndex,
) -> anyhow::Result<()> {
    if waiting.is_empty() {
        return Ok(());
    }
    // The digests of a commit stay until the last waiting commit on it.
    let mut children = BTreeMap::<&str, usize>::new();
    for (_, base) in waiting {
        if let Some(base) = base {
            *children.entry(base.as_str()).or_default() += 1;
        }
    }
    let mut maps = BTreeMap::<String, BTreeMap<String, String>>::new();
    let mut rows = Vec::<(String, Vec<(String, Option<Held>)>)>::new();
    let mut failed = None;
    let mut next = waiting.iter();
    let commits = waiting
        .iter()
        .map(|(file, _)| file.commit.as_str())
        .collect::<Vec<_>>();
    each_text(history, paths.records, &commits, |text| {
        let Some((file, base)) = next.next() else {
            return;
        };
        let before = base.as_ref().map_or_else(BTreeMap::new, |base| {
            maps.get(base)
                .cloned()
                .unwrap_or_else(|| index.stored(base))
        });
        let mut now = BTreeMap::new();
        let mut changes = Vec::new();
        for (record, value) in records(text.as_deref()) {
            match Held::of(kind, &value) {
                Ok(held) => {
                    if before.get(&record) != Some(&held.digest) {
                        changes.push((record.clone(), Some(held.clone())));
                    }
                    now.insert(record, held.digest);
                }
                Err(error) => {
                    failed.get_or_insert(error);
                }
            }
        }
        for record in before.keys().filter(|record| !now.contains_key(*record)) {
            changes.push((record.clone(), None));
        }
        if let Some(base) = base {
            if let Some(left) = children.get_mut(base.as_str()) {
                *left -= 1;
                if *left == 0 {
                    maps.remove(base);
                }
            }
        }
        if children.contains_key(file.commit.as_str()) {
            maps.insert(file.commit.clone(), now);
        }
        rows.push((file.commit.clone(), changes));
    })
    .await?;
    if let Some(error) = failed {
        return Err(error);
    }
    let held = rows
        .iter()
        .filter(|(_, changes)| changes.iter().any(|(_, held)| held.is_some()))
        .map(|(commit, _)| commit.clone())
        .collect::<Vec<_>>();
    let commits = held.iter().map(String::as_str).collect::<Vec<_>>();
    let mut found = Vec::with_capacity(held.len());
    each_text(history, paths.discussions, &commits, |text| {
        found.push(origins::outcomes(text.as_deref(), kind));
    })
    .await?;
    let mut found = held.into_iter().zip(found).collect::<BTreeMap<_, _>>();
    for ((file, base), (commit, mut changes)) in waiting.iter().zip(rows) {
        let mut outcomes = found.remove(&commit).unwrap_or_default();
        for (record, held) in &mut changes {
            if let Some(held) = held {
                held.outcomes = outcomes.remove(record).unwrap_or_default();
            }
        }
        index.put(&file.commit, base.clone(), id, changes);
    }
    Ok(())
}
