//! The merge check of the review submissions in a Proposal file.
//!
//! Two branches from one base give their review submissions of one record the
//! same review cycle, and each submission has its own ID. So the merge by ID
//! keeps both, and each read of the record then refuses. The check finds these
//! submissions with the reading of the review rows and reports them as a
//! conflict, so that git leaves the file unmerged for a person.
//!
//! The merge keeps both submissions. A merge driver sees one file, but the
//! decisions and the withdrawals of a submission are in other files. When the
//! person accepts the file as written, the read of the record refuses and
//! names each line of each review file that the repair removes.

use super::validation::ShardFamily;
use super::{
    stored_row, with_side, CanonicalRecord, MergeConflict, MergeConflictKind, MergeOutcome,
    StoredRow,
};
use crate::jsonl::record_line_value;
use crate::state_store::message_ids::ShardLine;
use camino::Utf8Path;
use provenance_core::review::{read_review_rows, ReviewRowFault, StoredReviewRows};
use provenance_core::ProposalCard;
use provenance_macros::rule;
use std::collections::BTreeSet;

/// Reports a `same_review_cycle` conflict for a submission of their side that
/// shares a record and a review cycle with ours, and keeps both submissions.
#[rule("rule_merge_reports_same_review_cycle")]
pub fn review_cycle_conflicts(
    shard_path: &Utf8Path,
    ours: &[StoredRow],
    theirs: &[StoredRow],
    outcome: MergeOutcome<Vec<CanonicalRecord>>,
) -> anyhow::Result<MergeOutcome<Vec<CanonicalRecord>>> {
    if ShardFamily::for_shard_path(shard_path) != ShardFamily::Proposals {
        return Ok(outcome);
    }
    let (records, mut conflicts) = match outcome {
        MergeOutcome::Clean { records } => (records, Vec::new()),
        MergeOutcome::Conflicted { conflicts, partial } => (partial, conflicts),
    };
    let cards = records
        .iter()
        .map(|record| proposal(shard_path, ours, theirs, record))
        .collect::<anyhow::Result<Vec<_>>>()?;
    let our_ids: BTreeSet<&str> = ours.iter().filter_map(|row| id_of(row.record())).collect();
    let reported: BTreeSet<String> = conflicts.iter().map(|c| c.record_id.clone()).collect();
    let reading = read_review_rows(StoredReviewRows {
        proposals: &cards,
        dispositions: &[],
        withdrawals: &[],
    });
    for fault in reading.faults() {
        let ReviewRowFault::OneCycleTwoSubmissions { submissions, .. } = fault else {
            continue;
        };
        let positions = submissions.iter().map(|lines| lines.submission.position);
        let (kept, added): (Vec<usize>, Vec<usize>) =
            positions.partition(|&at| our_ids.contains(cards[at].id.as_str()));
        let Some(&kept) = kept.first() else {
            continue;
        };
        for at in added {
            if reported.contains(cards[at].id.as_str()) {
                continue;
            }
            conflicts.push(MergeConflict {
                kind: MergeConflictKind::SameReviewCycle,
                record_id: cards[at].id.as_str().to_owned(),
                base: None,
                ours: Some(records[kept].clone()),
                theirs: Some(records[at].clone()),
            });
        }
    }
    Ok(if conflicts.is_empty() {
        MergeOutcome::Clean { records }
    } else {
        MergeOutcome::Conflicted {
            conflicts,
            partial: records,
        }
    })
}

fn id_of(record: &CanonicalRecord) -> Option<&str> {
    record.get("id").and_then(serde_json::Value::as_str)
}

/// The Proposal of a merged record, read from the stored line of the side
/// that holds it.
fn proposal(
    shard_path: &Utf8Path,
    ours: &[StoredRow],
    theirs: &[StoredRow],
    record: &CanonicalRecord,
) -> anyhow::Result<ProposalCard> {
    let (side, row) = stored_row(ours, theirs, record)?;
    let line = row.line_number;
    let (_, value) = record_line_value::<ProposalCard>(shard_path, line, row.line.as_bytes())
        .map_err(|error| with_side(error, shard_path, line, side))?;
    serde_json::from_value(value).map_err(|error| {
        crate::refusal::state_fault(
            &ShardLine::new(shard_path, line),
            format!("{side}: {error}"),
        )
    })
}
