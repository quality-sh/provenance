//! The reading of the stored review rows: which rows read as one row, and
//! which rows do not agree.

use super::row_groups::{group_by_id, IdGroup};
use super::{
    review_cycle, submission_order, target, RecordKey, RecordRows, ReviewReading,
    ReviewRowFamily, ReviewRowFault, RowAt, SubmissionRows,
};
use crate::{DispositionDecision, DispositionRecord, ProposalCard, ProposalType, StableId};
use provenance_macros::rule;
use std::collections::HashMap;

impl<'a> ReviewReading<'a> {
    fn record_rows(&mut self, (kind, id): RecordKey<'a>) -> &mut RecordRows {
        self.records.entry(kind).or_default().entry(id).or_default()
    }

    fn fault(&mut self, fault: ReviewRowFault, records: &[RecordKey<'a>]) {
        let index = self.faults.len();
        self.faults.push(fault);
        if records.is_empty() {
            self.scope_fault.get_or_insert(index);
        }
        for record in records {
            self.record_rows(*record).fault.get_or_insert(index);
        }
    }

    /// The records that the submissions with this ID name.
    fn records_of_submission(&self, id: &str) -> Vec<RecordKey<'a>> {
        let proposals = self.rows.proposals;
        let mut records = Vec::new();
        for &position in self.submission_ids.get(id).into_iter().flatten() {
            let record = target(&proposals[position]);
            if !records.contains(&record) {
                records.push(record);
            }
        }
        records
    }

    fn records_of_lines<T>(
        &self,
        rows: &'a [T],
        group: &IdGroup,
        names: impl Fn(&'a T) -> &'a str,
    ) -> Vec<RecordKey<'a>> {
        let mut records = Vec::new();
        for &position in &group.positions {
            for record in self.records_of_submission(names(&rows[position])) {
                if !records.contains(&record) {
                    records.push(record);
                }
            }
        }
        records
    }

    pub(super) fn read_submissions(&mut self) {
        let proposals = self.rows.proposals;
        let review = |proposal: &ProposalCard| proposal.proposal_type == ProposalType::RecordRevision;
        let mut kept = Vec::new();
        for (position, proposal) in proposals.iter().enumerate() {
            if review(proposal) {
                self.submission_ids
                    .entry(proposal.id.as_str())
                    .or_default()
                    .push(position);
            } else {
                kept.push(position);
            }
        }
        let groups = group_by_id(proposals, |proposal| {
            review(proposal).then_some(proposal.id.as_str())
        });
        for group in groups {
            let first = proposals[group.positions[0]].id.as_str();
            if let Some(second) = group.differing {
                let records = self.records_of_submission(first);
                self.fault(
                    id_on_two_lines(
                        &proposals[group.positions[0]].id,
                        ReviewRowFamily::Submission,
                        group.positions[0],
                        second,
                    ),
                    &records,
                );
                continue;
            }
            let position = group.positions[0];
            kept.push(position);
            self.submissions
                .insert(first, (position, SubmissionRows::default()));
            self.record_rows(target(&proposals[position]))
                .submissions
                .push(position);
        }
        kept.sort_unstable();
        self.once.proposals = kept;
        for record in self.records.values_mut().flat_map(HashMap::values_mut) {
            record
                .submissions
                .sort_by_key(|&position| submission_order(&proposals[position]));
        }
    }

    pub(super) fn read_decisions(&mut self) {
        let dispositions = self.rows.dispositions;
        let mut kept = Vec::new();
        let mut others = Vec::new();
        for (position, disposition) in dispositions.iter().enumerate() {
            if !self.is_submission(&disposition.proposal_id) {
                kept.push(position);
                others.push(position);
            }
        }
        let groups = group_by_id(dispositions, |disposition| {
            self.is_submission(&disposition.proposal_id)
                .then_some(disposition.id.as_str())
        });
        for group in groups {
            let first = group.positions[0];
            if let Some(second) = group.differing {
                let records =
                    self.records_of_lines(dispositions, &group, |row| row.proposal_id.as_str());
                let fault =
                    id_on_two_lines(&dispositions[first].id, ReviewRowFamily::Decision, first, second);
                self.fault(fault, &records);
                continue;
            }
            kept.push(first);
            if let Some((_, rows)) = self
                .submissions
                .get_mut(dispositions[first].proposal_id.as_str())
            {
                rows.decisions.push(first);
            }
        }
        kept.sort_unstable();
        self.once.dispositions = kept;
        self.read_other_decisions(&others);
    }

    /// Adds each decision on a Proposal that is not a review submission to
    /// the record that its Proposal names, so that the history shows it.
    fn read_other_decisions(&mut self, others: &[usize]) {
        let all = self.rows.proposals;
        let mut proposals = HashMap::new();
        for (position, proposal) in all.iter().enumerate().rev() {
            proposals.insert(proposal.id.as_str(), position);
        }
        for &position in others {
            let disposition = &self.rows.dispositions[position];
            if let Some(&proposal) = proposals.get(disposition.proposal_id.as_str()) {
                self.record_rows(target(&all[proposal]))
                    .other_decisions
                    .push((position, proposal));
            }
        }
    }

    pub(super) fn read_withdrawals(&mut self) {
        let withdrawals = self.rows.withdrawals;
        let mut kept = Vec::new();
        for group in group_by_id(withdrawals, |withdrawal| Some(withdrawal.id.as_str())) {
            let first = group.positions[0];
            if let Some(second) = group.differing {
                let records =
                    self.records_of_lines(withdrawals, &group, |row| row.proposal_id.as_str());
                let fault = id_on_two_lines(
                    &withdrawals[first].id,
                    ReviewRowFamily::Withdrawal,
                    first,
                    second,
                );
                self.fault(fault, &records);
                continue;
            }
            kept.push(first);
            let withdrawal = &withdrawals[first];
            if !self.is_submission(&withdrawal.proposal_id) {
                let fault = ReviewRowFault::NoSubmission {
                    withdrawal: RowAt {
                        family: ReviewRowFamily::Withdrawal,
                        position: first,
                    },
                    proposal: withdrawal.proposal_id.clone(),
                };
                self.fault(fault, &[]);
            } else if let Some((_, rows)) =
                self.submissions.get_mut(withdrawal.proposal_id.as_str())
            {
                rows.withdrawals.push(first);
            }
        }
        kept.sort_unstable();
        self.once.withdrawals = kept;
    }

    pub(super) fn read_records(&mut self) {
        let proposals = self.rows.proposals;
        let mut records = self
            .records
            .iter()
            .flat_map(|(&kind, ids)| ids.iter().map(move |(&id, rows)| ((kind, id), rows)))
            .filter_map(|(key, rows)| Some((key, *rows.submissions.iter().min()?)))
            .collect::<Vec<_>>();
        records.sort_by_key(|(_, first)| *first);
        for (key, _) in records {
            let submissions = self.records[key.0][key.1].submissions.clone();
            let mut faults = one_cycle_two_submissions(proposals, &submissions)
                .into_iter()
                .collect::<Vec<_>>();
            let mut in_line_order = submissions;
            in_line_order.sort_unstable();
            for position in in_line_order {
                let (_, rows) = &self.submissions[proposals[position].id.as_str()];
                faults.extend(record_fault(
                    &proposals[position].id,
                    self.rows.dispositions,
                    rows,
                ));
            }
            for fault in faults {
                self.fault(fault, &[key]);
            }
        }
    }

}

fn id_on_two_lines(
    id: &StableId,
    family: ReviewRowFamily,
    first: usize,
    second: usize,
) -> ReviewRowFault {
    ReviewRowFault::IdOnTwoLines {
        id: id.clone(),
        first: RowAt {
            family,
            position: first,
        },
        second: RowAt {
            family,
            position: second,
        },
    }
}

/// Two submissions of one record with one review cycle, the first pair in
/// line order.
fn one_cycle_two_submissions(
    proposals: &[ProposalCard],
    submissions: &[usize],
) -> Option<ReviewRowFault> {
    let mut in_line_order = submissions.to_vec();
    in_line_order.sort_unstable();
    let mut cycles = HashMap::new();
    for position in in_line_order {
        let Some(cycle) = review_cycle(&proposals[position]) else {
            continue;
        };
        if let Some(&first) = cycles.get(&cycle) {
            let at = |position| RowAt {
                family: ReviewRowFamily::Submission,
                position,
            };
            return Some(ReviewRowFault::OneCycleTwoSubmissions {
                cycle,
                first: at(first),
                second: at(position),
            });
        }
        cycles.insert(cycle, position);
    }
    None
}

/// The decisions or the withdrawals of one submission that do not agree.
#[rule("rule_disagreeing_review_rows_refuse_record")]
fn record_fault(
    submission: &StableId,
    dispositions: &[DispositionRecord],
    rows: &SubmissionRows,
) -> Option<ReviewRowFault> {
    let decision = |position| RowAt {
        family: ReviewRowFamily::Decision,
        position,
    };
    if let Some((&first, rest)) = rows.decisions.split_first() {
        let result = dispositions[first].decision;
        if let Some(&second) = rest
            .iter()
            .find(|&&position| dispositions[position].decision != result)
        {
            return Some(ReviewRowFault::DifferentResults {
                submission: submission.clone(),
                first: decision(first),
                second: decision(second),
            });
        }
    }
    let approval = rows
        .decisions
        .iter()
        .find(|&&position| dispositions[position].decision == DispositionDecision::Accepted);
    match (approval, rows.withdrawals.first()) {
        (Some(&approval), Some(&withdrawal)) => Some(ReviewRowFault::AcceptedAndWithdrawn {
            submission: submission.clone(),
            approval: decision(approval),
            withdrawal: RowAt {
                family: ReviewRowFamily::Withdrawal,
                position: withdrawal,
            },
        }),
        _ => None,
    }
}
