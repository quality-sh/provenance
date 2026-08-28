use crate::state_store::readers::{
    ensure_supported_ideation_landing_versions, ensure_supported_record_version,
};
use provenance_macros::rule;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub mod rbac;
pub mod validation;

pub use rbac::validate_rbac_merged_records;
pub use validation::{changed_statement_diagnostics, validate_merged_records, ShardFamily};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeConflictKind {
    /// Both sides added the same id with different content. There is no base
    /// pre-image to compare against, so neither side can be called the mover.
    AddAdd,
    /// Both sides changed a record that existed in the base, differently.
    DivergentEdit,
    /// One side deleted a record the other side changed.
    DeleteModify,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct MergeConflict {
    pub kind: MergeConflictKind,
    pub record_id: String,
    /// The base pre-image, so a reader can see what both sides moved away
    /// from. `None` for an add/add clash, where the base held nothing.
    pub base: Option<CanonicalRecord>,
    pub ours: Option<Value>,
    pub theirs: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum MergeOutcome<T> {
    Clean {
        records: T,
    },
    Conflicted {
        conflicts: Vec<MergeConflict>,
        partial: T,
    },
}

impl<T> MergeOutcome<T> {
    pub fn unwrap_clean(self) -> T {
        match self {
            Self::Clean { records } => records,
            Self::Conflicted { .. } => panic!("expected clean merge"),
        }
    }

    pub fn unwrap_conflicts(self) -> Vec<MergeConflict> {
        match self {
            Self::Clean { .. } => panic!("expected conflicted merge"),
            Self::Conflicted { conflicts, .. } => conflicts,
        }
    }
}

pub type CanonicalRecord = Value;

/// Merges three sides of one JSONL shard by record id.
///
/// This decides *which* record survives, not whether the survivor is a legal
/// record: it merges untyped JSON and never inspects a field other than `id`.
/// A merged set can therefore hold a record that no writer would have accepted,
/// because each side's record was legal on its own branch and only the
/// combination is not. The caller must put the merged records back through the
/// write gate before they land: [`validate_merged_records`] re-checks them
/// against the type the shard holds, and the merge fails rather than storing an
/// invalid record.
#[rule("rule_record_merge")]
pub fn merge_records(
    base: &[CanonicalRecord],
    ours: &[CanonicalRecord],
    theirs: &[CanonicalRecord],
) -> anyhow::Result<MergeOutcome<Vec<CanonicalRecord>>> {
    let base = index_by_id(base)?;
    let ours = index_by_id(ours)?;
    let theirs = index_by_id(theirs)?;
    let mut ids = BTreeSet::new();
    ids.extend(base.keys().cloned());
    ids.extend(ours.keys().cloned());
    ids.extend(theirs.keys().cloned());

    let mut merged = Vec::new();
    let mut conflicts = Vec::new();
    for id in ids {
        match (base.get(&id), ours.get(&id), theirs.get(&id)) {
            (None, Some(ours), None) => merged.push(ours.clone()),
            (None, None, Some(theirs)) => merged.push(theirs.clone()),
            (None, Some(ours), Some(theirs)) if ours == theirs => merged.push(ours.clone()),
            (Some(_), None, None) => {}
            (Some(base), Some(ours), None) if ours == base => {}
            (Some(base), Some(ours), None) => {
                merged.push(ours.clone());
                conflicts.push(MergeConflict {
                    kind: MergeConflictKind::DeleteModify,
                    record_id: id,
                    base: Some(base.clone()),
                    ours: Some(ours.clone()),
                    theirs: None,
                });
            }
            (Some(base), None, Some(theirs)) if theirs == base => {}
            (Some(base), None, Some(theirs)) => {
                merged.push(theirs.clone());
                conflicts.push(MergeConflict {
                    kind: MergeConflictKind::DeleteModify,
                    record_id: id,
                    base: Some(base.clone()),
                    ours: None,
                    theirs: Some(theirs.clone()),
                });
            }
            (Some(_), Some(ours), Some(theirs)) if ours == theirs => merged.push(ours.clone()),
            (Some(base), Some(ours), Some(theirs)) if ours == base => merged.push(theirs.clone()),
            (Some(base), Some(ours), Some(theirs)) if theirs == base => merged.push(ours.clone()),
            (base, Some(ours), Some(theirs)) => {
                merged.push(ours.clone());
                conflicts.push(MergeConflict {
                    kind: if base.is_none() {
                        MergeConflictKind::AddAdd
                    } else {
                        MergeConflictKind::DivergentEdit
                    },
                    record_id: id,
                    base: base.cloned(),
                    ours: Some(ours.clone()),
                    theirs: Some(theirs.clone()),
                });
            }
            (None, None, None) => unreachable!(),
        }
    }

    if conflicts.is_empty() {
        Ok(MergeOutcome::Clean { records: merged })
    } else {
        Ok(MergeOutcome::Conflicted {
            conflicts,
            partial: merged,
        })
    }
}

pub fn read_jsonl_records(path: &camino::Utf8Path) -> anyhow::Result<Vec<CanonicalRecord>> {
    read_jsonl_records_for_shard(path, path)
}

pub fn read_jsonl_records_for_shard(
    path: &camino::Utf8Path,
    shard_path: &camino::Utf8Path,
) -> anyhow::Result<Vec<CanonicalRecord>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let contents = std::fs::read_to_string(path)?;
    let mut records = Vec::new();
    for (index, line) in contents.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let record = serde_json::from_str(line)?;
        // Errors name the logical shard, but line numbers come from Git's temporary input file.
        ensure_supported_record_version(shard_path, index + 1, &record)?;
        if ShardFamily::for_shard_path(shard_path) == ShardFamily::IdeationLandings {
            ensure_supported_ideation_landing_versions(shard_path, index + 1, &record)?;
        }
        records.push(record);
    }
    Ok(records)
}

fn index_by_id(records: &[CanonicalRecord]) -> anyhow::Result<BTreeMap<String, CanonicalRecord>> {
    let mut indexed = BTreeMap::new();
    for record in records {
        let id = record
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("record is missing string id"))?;
        anyhow::ensure!(!indexed.contains_key(id), "duplicate record id {id}");
        indexed.insert(id.to_string(), record.clone());
    }
    Ok(indexed)
}

#[cfg(test)]
mod tests;
