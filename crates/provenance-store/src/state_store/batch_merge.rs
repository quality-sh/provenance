//! Record-merge helpers for ideation landing batches, split out of
//! `ideation_batches` so the batch writer and its validation stay under the
//! file-size limit. Everything here is pure bookkeeping over record vecs:
//! duplicate detection, replace-vs-refuse semantics, and overlay ordering.

use super::IdeationLandingBatch;
use provenance_core::ScopeId;
use std::collections::{BTreeMap, BTreeSet};

pub fn insert_all<'a, T: serde::Serialize>(
    kind: &str,
    records: &'a [T],
    id: impl Fn(&'a T) -> &'a str,
    seen: &mut BTreeMap<String, serde_json::Value>,
) -> anyhow::Result<()> {
    for record in records {
        let record_id = id(record);
        let value = serde_json::to_value(record)?;
        anyhow::ensure!(
            seen.insert(record_id.to_owned(), value).is_none(),
            "duplicate immutable {kind} id {record_id}"
        );
    }
    Ok(())
}

pub fn ensure_scope(scope: &ScopeId, batch: &IdeationLandingBatch) -> anyhow::Result<()> {
    for (kind, actual) in batch
        .contributions
        .iter()
        .map(|r| ("contribution", &r.scope_id))
        .chain(
            batch
                .synthesis_packets
                .iter()
                .map(|r| ("synthesis packet", &r.scope_id)),
        )
        .chain(batch.proposals.iter().map(|r| ("proposal", &r.scope_id)))
        .chain(batch.assertions.iter().map(|r| ("assertion", &r.scope_id)))
        .chain(
            batch
                .dispositions
                .iter()
                .map(|r| ("disposition", &r.scope_id)),
        )
    {
        anyhow::ensure!(actual == scope, "{kind} scope_id must match landing scope");
    }
    Ok(())
}

pub fn merge_replaceable<T: Clone>(
    kind: &str,
    existing: &mut Vec<T>,
    incoming: &[T],
    replace: bool,
    id: impl Fn(&T) -> &str,
) -> anyhow::Result<()> {
    let mut incoming_ids = BTreeSet::new();
    for record in incoming {
        let record_id = id(record);
        anyhow::ensure!(
            incoming_ids.insert(record_id),
            "duplicate {kind} id {record_id} in batch"
        );
        if let Some(index) = existing.iter().position(|current| id(current) == record_id) {
            anyhow::ensure!(replace, "{kind} {record_id} already exists");
            existing[index] = record.clone();
        } else {
            existing.push(record.clone());
        }
    }
    Ok(())
}

pub fn merge_immutable<T: Clone>(
    kind: &str,
    existing: &mut Vec<T>,
    incoming: &[T],
    id: impl Fn(&T) -> &str,
) -> anyhow::Result<()> {
    let mut incoming_ids = BTreeSet::new();
    for record in incoming {
        let record_id = id(record);
        anyhow::ensure!(
            incoming_ids.insert(record_id),
            "duplicate {kind} id {record_id} in batch"
        );
        anyhow::ensure!(
            !existing.iter().any(|current| id(current) == record_id),
            "{kind} {record_id} already exists and is immutable"
        );
        existing.push(record.clone());
    }
    Ok(())
}

pub fn overlay_records<T>(records: &mut Vec<T>, incoming: Vec<T>, id: impl Fn(&T) -> &str) {
    for record in incoming {
        if let Some(index) = records
            .iter()
            .position(|current| id(current) == id(&record))
        {
            records[index] = record;
        } else {
            records.push(record);
        }
    }
    records.sort_by(|a, b| id(a).cmp(id(b)));
}
