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

/// `1`, `1 and 2` or `1, 2 and 3`.
fn joined(items: impl IntoIterator<Item = String>) -> String {
    let mut items = items.into_iter().collect::<Vec<_>>();
    let Some(last) = items.pop() else {
        return String::new();
    };
    if items.is_empty() {
        last
    } else {
        format!("{} and {last}", items.join(", "))
    }
}

/// The lines of `at`, file by file in the order of the rows:
/// `ideation/proposal_cards.jsonl line 3; ideation/dispositions.jsonl lines
/// 1 and 4`.
fn lines_text(rows: &StoredRows, at: &[RowAt]) -> String {
    let mut files: Vec<(String, Vec<String>)> = Vec::new();
    for row in at {
        let line = rows.line(*row);
        let file = file_of(line);
        let number = line.line.to_string();
        match files.iter_mut().find(|(known, _)| *known == file) {
            Some((_, numbers)) => numbers.push(number),
            None => files.push((file, vec![number])),
        }
    }
    files
        .into_iter()
        .map(|(file, numbers)| {
            let word = if numbers.len() == 1 { "line" } else { "lines" };
            format!("{file} {word} {}", joined(numbers))
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// `D1 at line 1 and D2 at line 4`, for rows of one file.
fn rows_text(rows: &StoredRows, at: &[RowAt], label: impl Fn(RowAt) -> String) -> String {
    joined(
        at.iter()
            .map(|&row| format!("{} at line {}", label(row), rows.line(row).line)),
    )
}

/// The message of a fault. Its repair names each line that the person
/// removes, so that one repair is sufficient.
#[rule("rule_review_row_refusal_gives_repair")]
fn message(rows: &StoredRows, fault: &ReviewRowFault) -> String {
    match fault {
        ReviewRowFault::IdOnTwoLines { id, lines, .. } => {
            let numbers = lines.iter().map(|&at| rows.line(at).line.to_string());
            format!(
                "{} {} is on lines {} of {} with different contents. To repair, keep one of \
                 these lines and remove the others, then commit.",
                noun(lines[0].family),
                id.as_str(),
                joined(numbers),
                file_of(rows.line(lines[0])),
            )
        }
        ReviewRowFault::DifferentResults {
            submission,
            decisions,
            ..
        } => format!(
            "{} has decisions with different results in {}: {}. To repair, remove each of these \
             lines that has a result that you do not keep, then commit.",
            submission_name(rows, submission),
            file_of(rows.line(decisions[0])),
            rows_text(rows, decisions, |at| format!(
                "{} {}",
                result_word(rows, at),
                rows.id(at).as_str()
            )),
        ),
        ReviewRowFault::AcceptedAndWithdrawn {
            submission,
            approvals,
            withdrawals,
        } => format!(
            "{} is accepted and withdrawn. The approvals in {}: {}. The withdrawals in {}: {}. \
             To repair, remove each approval line or each withdrawal line, then commit.",
            submission_name(rows, submission),
            file_of(rows.line(approvals[0])),
            rows_text(rows, approvals, |at| rows.id(at).as_str().to_owned()),
            file_of(rows.line(withdrawals[0])),
            rows_text(rows, withdrawals, |at| rows.id(at).as_str().to_owned()),
        ),
        ReviewRowFault::OneCycleTwoSubmissions { cycle, submissions } => {
            let target = &rows.proposals[submissions[0].submission.position]
                .traceability
                .target;
            let each = submissions
                .iter()
                .map(|lines| {
                    format!(
                        "The lines of {}: {}.",
                        rows.id(lines.submission).as_str(),
                        lines_text(rows, &lines.lines)
                    )
                })
                .collect::<Vec<_>>()
                .join(" ");
            format!(
                "{} {} has {} review submissions with review cycle {cycle}. To repair, keep one \
                 submission and remove the lines of each other submission, then commit. {}",
                NodeType::from(target.artifact_type).as_str(),
                target.artifact_id.as_str(),
                submissions.len(),
                each,
            )
        }
        ReviewRowFault::NoSubmission { proposal, lines } => format!(
            "withdrawal {} names no review submission: {}. To repair, remove {}, then commit.",
            rows.id(lines[0]).as_str(),
            proposal.as_str(),
            lines_text(rows, lines),
        ),
    }
}

/// `invalid_state_file` at the line of the row that the fault names.
#[rule("rule_inconsistent_records_refusal_names_records")]
pub(super) fn refusal(rows: &StoredRows, fault: &ReviewRowFault) -> anyhow::Error {
    state_fault(rows.line(fault.at()), message(rows, fault))
}
