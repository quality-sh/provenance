//! The merge check of the review submissions in a Proposal file.
//!
//! Two branches from one base give their review submissions of one record the
//! same review cycle, and each submission has its own ID. So the merge by ID
//! keeps both, and each read of the record then refuses. The check finds these
//! submissions with the reading of the review rows and reports them as a
//! conflict, so that git leaves the file unmerged for a person.

use super::validation::is_scoped_family;
use super::{CanonicalRecord, MergeConflict, MergeConflictKind, MergeOutcome};
use anyhow::Context;
use camino::Utf8Path;
use provenance_core::review::{read_review_rows, ReviewRowFault, StoredReviewRows};
use provenance_core::ProposalCard;
use std::collections::BTreeSet;

/// Whether the shard at `path` is the Proposal file of a scope.
fn is_proposal_file(path: &Utf8Path) -> bool {
    path.file_name() == Some("proposal_cards.jsonl") && is_scoped_family(path, "ideation")
}

fn id_of(record: &CanonicalRecord) -> Option<&str> {
    record.get("id").and_then(serde_json::Value::as_str)
}

/// Adds a conflict for each review submission of their side that the merge
/// keeps beside a submission of our side with the same record and review
/// cycle, and keeps only our submission. A group of submissions that our
/// side alone already holds is not the doing of the merge and stays.
pub fn review_cycle_conflicts(
    shard_path: &Utf8Path,
    ours: &[CanonicalRecord],
    outcome: MergeOutcome<Vec<CanonicalRecord>>,
) -> anyhow::Result<MergeOutcome<Vec<CanonicalRecord>>> {
    if !is_proposal_file(shard_path) {
        return Ok(outcome);
    }
    let (records, mut conflicts) = match outcome {
        MergeOutcome::Clean { records } => (records, Vec::new()),
        MergeOutcome::Conflicted { conflicts, partial } => (partial, conflicts),
    };
    let cards = records
        .iter()
        .map(|record| serde_json::from_value::<ProposalCard>(record.clone()))
        .collect::<Result<Vec<_>, _>>()
        .with_context(|| format!("a merged line of {shard_path} is not a Proposal"))?;
    let ours: BTreeSet<&str> = ours.iter().filter_map(id_of).collect();
    let reading = read_review_rows(StoredReviewRows {
        proposals: &cards,
        dispositions: &[],
        withdrawals: &[],
    });
    let mut dropped = BTreeSet::new();
    for fault in reading.faults() {
        let ReviewRowFault::OneCycleTwoSubmissions { submissions, .. } = fault else {
            continue;
        };
        let positions = submissions.iter().map(|lines| lines.submission.position);
        let (kept, added): (Vec<usize>, Vec<usize>) =
            positions.partition(|&at| ours.contains(cards[at].id.as_str()));
        let Some(&kept) = kept.first() else {
            continue;
        };
        for at in added {
            conflicts.push(MergeConflict {
                kind: MergeConflictKind::SameReviewCycle,
                record_id: cards[at].id.as_str().to_owned(),
                base: None,
                ours: Some(records[kept].clone()),
                theirs: Some(records[at].clone()),
            });
            dropped.insert(at);
        }
    }
    if conflicts.is_empty() {
        return Ok(MergeOutcome::Clean { records });
    }
    let partial = records
        .into_iter()
        .enumerate()
        .filter(|(at, _)| !dropped.contains(at))
        .map(|(_, record)| record)
        .collect();
    Ok(MergeOutcome::Conflicted { conflicts, partial })
}
