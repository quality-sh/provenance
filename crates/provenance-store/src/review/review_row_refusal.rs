//! The refusal of review rows that do not agree: `invalid_state_file` at the
//! line of the second row, with a message that names both rows and the step
//! that repairs them.

use super::review_rows::StoredRows;
use crate::refusal::state_fault;
use crate::state_store::message_ids::ShardLine;
use provenance_core::{
    review::{ReviewRowFamily, ReviewRowFault, RowAt},
    NodeType, ProposalCard, StableId,
};
use provenance_macros::rule;

/// The file of a row below the scope directory, as `ideation/<file>`.
fn file_of(at: &ShardLine) -> String {
    let path = at.path.as_path();
    match (
        path.parent().and_then(|parent| parent.file_name()),
        path.file_name(),
    ) {
        (Some(directory), Some(file)) => format!("{directory}/{file}"),
        _ => path.to_string(),
    }
}

impl StoredRows {
    fn line(&self, at: RowAt) -> &ShardLine {
        let lines = match at.family {
            ReviewRowFamily::Submission => &self.lines.proposals,
            ReviewRowFamily::Decision => &self.lines.dispositions,
            ReviewRowFamily::Withdrawal => &self.lines.withdrawals,
        };
        &lines[at.position]
    }

    fn id(&self, at: RowAt) -> &StableId {
        match at.family {
            ReviewRowFamily::Submission => &self.proposals[at.position].id,
            ReviewRowFamily::Decision => &self.dispositions[at.position].id,
            ReviewRowFamily::Withdrawal => &self.withdrawals[at.position].id,
        }
    }

    /// The submission that names `id`, if one line of it reads.
    fn submission(&self, id: &StableId) -> Option<&ProposalCard> {
        self.proposals.iter().find(|proposal| proposal.id == *id)
    }
}

const fn noun(family: ReviewRowFamily) -> &'static str {
    match family {
        ReviewRowFamily::Submission => "review submission",
        ReviewRowFamily::Decision => "decision",
        ReviewRowFamily::Withdrawal => "withdrawal",
    }
}

/// `review submission S of requirement req_a`.
fn submission_name(rows: &StoredRows, submission: &StableId) -> String {
    let record = rows.submission(submission).map(|proposal| {
        let target = &proposal.traceability.target;
        format!(
            " of {} {}",
            NodeType::from(target.artifact_type).as_str(),
            target.artifact_id.as_str()
        )
    });
    format!(
        "review submission {}{}",
        submission.as_str(),
        record.unwrap_or_default()
    )
}

fn result_word(rows: &StoredRows, at: RowAt) -> &'static str {
    match rows.dispositions[at.position].decision {
        provenance_core::DispositionDecision::Accepted => "accepted",
        provenance_core::DispositionDecision::Rejected => "rejected",
        provenance_core::DispositionDecision::Deferred => "deferred",
    }
}

/// The message of a fault and the row whose line the refusal names.
#[rule("rule_review_row_refusal_gives_repair")]
fn message(rows: &StoredRows, fault: &ReviewRowFault) -> (String, RowAt) {
    match fault {
        ReviewRowFault::IdOnTwoLines { id, first, second } => (
            format!(
                "{} {} is on line {} and line {} of {} with different contents. To repair, \
                 remove the line that is not correct, then commit.",
                noun(first.family),
                id.as_str(),
                rows.line(*first).line,
                rows.line(*second).line,
                file_of(rows.line(*second)),
            ),
            *second,
        ),
        ReviewRowFault::DifferentResults {
            submission,
            first,
            second,
        } => (
            format!(
                "{} has two decisions with different results: {} {} at {} line {} and {} {} at \
                 line {}. To repair, remove the line of the decision that you do not keep, then \
                 commit.",
                submission_name(rows, submission),
                result_word(rows, *first),
                rows.id(*first).as_str(),
                file_of(rows.line(*first)),
                rows.line(*first).line,
                result_word(rows, *second),
                rows.id(*second).as_str(),
                rows.line(*second).line,
            ),
            *second,
        ),
        ReviewRowFault::AcceptedAndWithdrawn {
            submission,
            approval,
            withdrawal,
        } => (
            format!(
                "{} is accepted by {} at {} line {} and withdrawn by {} at {} line {}. To \
                 repair, remove the line of the approval or of the withdrawal, then commit.",
                submission_name(rows, submission),
                rows.id(*approval).as_str(),
                file_of(rows.line(*approval)),
                rows.line(*approval).line,
                rows.id(*withdrawal).as_str(),
                file_of(rows.line(*withdrawal)),
                rows.line(*withdrawal).line,
            ),
            *withdrawal,
        ),
        ReviewRowFault::OneCycleTwoSubmissions {
            cycle,
            first,
            second,
        } => {
            let target = &rows.proposals[first.position].traceability.target;
            let file = file_of(rows.line(*first));
            let directory = file.split_once('/').map_or("", |(directory, _)| directory);
            (
                format!(
                    "{} {} has two review submissions with review cycle {cycle}: {} at {file} \
                     line {} and {} at line {}. To repair, remove the line of one submission \
                     and each line of {directory}/dispositions.jsonl and \
                     {directory}/withdrawals.jsonl that names it, then commit.",
                    NodeType::from(target.artifact_type).as_str(),
                    target.artifact_id.as_str(),
                    rows.id(*first).as_str(),
                    rows.line(*first).line,
                    rows.id(*second).as_str(),
                    rows.line(*second).line,
                ),
                *second,
            )
        }
        ReviewRowFault::NoSubmission {
            withdrawal,
            proposal,
        } => (
            format!(
                "withdrawal {} names no review submission: {}",
                rows.id(*withdrawal).as_str(),
                proposal.as_str()
            ),
            *withdrawal,
        ),
    }
}

/// `invalid_state_file` at the line of the second row of `fault`.
#[rule("rule_inconsistent_records_refusal_names_records")]
pub(super) fn refusal(rows: &StoredRows, fault: &ReviewRowFault) -> anyhow::Error {
    let (message, at) = message(rows, fault);
    state_fault(rows.line(at), message)
}
