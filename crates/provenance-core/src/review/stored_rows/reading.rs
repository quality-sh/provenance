//! The reading of the stored review rows: which rows read as one row, and
//! which rows do not agree.

use super::row_groups::{group_by_id, IdGroup};
use super::{
    is_review_submission, review_cycle, submission_order, target, RecordKey, RecordRows,
    ReviewReading, ReviewRowFamily, ReviewRowFault, RowAt, SubmissionLines, SubmissionRows,
};
use crate::{DispositionDecision, StableId};
use std::collections::{HashMap, HashSet};

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
    pub(super) fn records_of_submission(&self, id: &str) -> Vec<RecordKey<'a>> {
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
        let mut kept = Vec::new();
        for (position, proposal) in proposals.iter().enumerate() {
            if is_review_submission(proposal) {
                self.submission_ids
                    .entry(proposal.id.as_str())
                    .or_default()
                    .push(position);
                self.record_rows(target(proposal)).lines.push(position);
            } else {
                kept.push(position);
            }
        }
        let groups = group_by_id(proposals, |proposal| {
            is_review_submission(proposal).then_some(proposal.id.as_str())
        });
        for group in groups {
            let first = proposals[group.positions[0]].id.as_str();
            if let Some(second) = group.differing {
                let records = self.records_of_submission(first);
                let fault = id_on_lines(
                    &proposals[group.positions[0]].id,
                    ReviewRowFamily::Submission,
                    &group.positions,
                    second,
                );
                self.fault(fault, &records);
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
            let submission = disposition.proposal_id.as_str();
            if self.is_submission(&disposition.proposal_id) {
                let naming = self.naming.entry(submission).or_default();
                naming.decisions.push(position);
            } else {
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
                let family = ReviewRowFamily::Decision;
                let fault = id_on_lines(&dispositions[first].id, family, &group.positions, second);
                self.fault(fault, &records);
                for &position in &group.positions {
                    let submission = dispositions[position].proposal_id.as_str();
                    self.named_by_disputed.insert(submission);
                }
                continue;
            }
            let submission = dispositions[first].proposal_id.as_str();
            if let Some((_, rows)) = self.submissions.get_mut(submission) {
                rows.decisions.push(first);
                kept.push(first);
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
        for (position, withdrawal) in withdrawals.iter().enumerate() {
            if self.is_submission(&withdrawal.proposal_id) {
                let submission = withdrawal.proposal_id.as_str();
                let naming = self.naming.entry(submission).or_default();
                naming.withdrawals.push(position);
            }
        }
        let mut kept = Vec::new();
        for group in group_by_id(withdrawals, |withdrawal| Some(withdrawal.id.as_str())) {
            let first = group.positions[0];
            if let Some(second) = group.differing {
                let records =
                    self.records_of_lines(withdrawals, &group, |row| row.proposal_id.as_str());
                let family = ReviewRowFamily::Withdrawal;
                let fault = id_on_lines(&withdrawals[first].id, family, &group.positions, second);
                self.fault(fault, &records);
                for &position in &group.positions {
                    let submission = withdrawals[position].proposal_id.as_str();
                    self.named_by_disputed.insert(submission);
                }
                self.read_lines_with_no_submission(&group.positions);
                continue;
            }
            let withdrawal = &withdrawals[first];
            if !self.is_submission(&withdrawal.proposal_id) {
                kept.push(first);
                self.read_lines_with_no_submission(&group.positions);
            } else if let Some((_, rows)) =
                self.submissions.get_mut(withdrawal.proposal_id.as_str())
            {
                rows.withdrawals.push(first);
                kept.push(first);
            }
        }
        kept.sort_unstable();
        self.once.withdrawals = kept;
    }

    /// Gives the fault of the scope for the lines of one withdrawal ID that
    /// name no review submission, if there are such lines.
    fn read_lines_with_no_submission(&mut self, positions: &[usize]) {
        let withdrawals = self.rows.withdrawals;
        let lines = positions
            .iter()
            .copied()
            .filter(|&position| !self.is_submission(&withdrawals[position].proposal_id))
            .collect::<Vec<_>>();
        if let Some(&first) = lines.first() {
            let fault = ReviewRowFault::NoSubmission {
                proposal: withdrawals[first].proposal_id.clone(),
                lines: rows_at(ReviewRowFamily::Withdrawal, &lines),
            };
            self.fault(fault, &[]);
        }
    }

    /// Gives the faults of each record: each review cycle that more than one
    /// of its submissions has, then the faults of the rows that name each
    /// of its submissions. The faults of a submission go to each record that
    /// a line of the submission names.
    pub(super) fn read_records(&mut self) {
        let proposals = self.rows.proposals;
        let mut records = self
            .records
            .iter()
            .flat_map(|(&kind, ids)| ids.iter().map(move |(&id, rows)| ((kind, id), rows)))
            .filter_map(|(key, rows)| Some((key, *rows.lines.first()?)))
            .collect::<Vec<_>>();
        records.sort_by_key(|(_, first)| *first);
        let mut read = HashSet::new();
        for (key, _) in records {
            let lines = self.records[key.0][key.1].lines.clone();
            for fault in self.shared_cycles(&lines) {
                self.fault(fault, &[key]);
            }
            for position in lines {
                let submission = &proposals[position].id;
                if !read.insert(submission.as_str()) {
                    continue;
                }
                let records = self.records_of_submission(submission.as_str());
                for fault in self.submission_faults(submission) {
                    self.fault(fault, &records);
                }
            }
        }
    }

    /// The lines of each decision and of each withdrawal that name the
    /// submission, by family, then in line order.
    fn lines_naming(&self, submission: &StableId) -> Vec<RowAt> {
        let Some(rows) = self.naming.get(submission.as_str()) else {
            return Vec::new();
        };
        let mut lines = rows_at(ReviewRowFamily::Decision, &rows.decisions);
        lines.extend(rows_at(ReviewRowFamily::Withdrawal, &rows.withdrawals));
        lines
    }

    /// One fault for each review cycle that more than one of the
    /// submissions has, in the line order of the second submission.
    /// `lines` are the submission lines of one record, in line order. A
    /// submission counts one time for each cycle of its lines, at the first
    /// line with that cycle.
    fn shared_cycles(&self, lines: &[usize]) -> Vec<ReviewRowFault> {
        let proposals = self.rows.proposals;
        let mut cycles: Vec<(u64, Vec<usize>)> = Vec::new();
        for &position in lines {
            let Some(cycle) = review_cycle(&proposals[position]) else {
                continue;
            };
            let id = &proposals[position].id;
            if let Some((_, positions)) = cycles.iter_mut().find(|(known, _)| *known == cycle) {
                if !positions.iter().any(|&other| proposals[other].id == *id) {
                    positions.push(position);
                }
            } else {
                cycles.push((cycle, vec![position]));
            }
        }
        cycles.retain(|(_, positions)| positions.len() > 1);
        cycles.sort_by_key(|(_, positions)| positions[1]);
        cycles
            .into_iter()
            .map(
                |(cycle, positions)| ReviewRowFault::OneCycleTwoSubmissions {
                    cycle,
                    submissions: positions
                        .into_iter()
                        .map(|position| self.submission_lines(position))
                        .collect(),
                },
            )
            .collect()
    }

    fn submission_lines(&self, position: usize) -> SubmissionLines {
        let id = &self.rows.proposals[position].id;
        let mut lines = rows_at(
            ReviewRowFamily::Submission,
            &self.submission_ids[id.as_str()],
        );
        lines.extend(self.lines_naming(id));
        SubmissionLines {
            submission: RowAt {
                family: ReviewRowFamily::Submission,
                position,
            },
            lines,
        }
    }

    /// The decisions or the withdrawals of one submission that do not agree.
    /// Each decision line and each withdrawal line that names the
    /// submission takes part.
    pub(super) fn submission_faults(&self, submission: &StableId) -> Vec<ReviewRowFault> {
        let Some(rows) = self.naming.get(submission.as_str()) else {
            return Vec::new();
        };
        let dispositions = self.rows.dispositions;
        let mut faults = Vec::new();
        if let Some((&first, rest)) = rows.decisions.split_first() {
            let result = dispositions[first].decision;
            if let Some(&second) = rest
                .iter()
                .find(|&&position| dispositions[position].decision != result)
            {
                faults.push(ReviewRowFault::DifferentResults {
                    submission: submission.clone(),
                    decisions: rows_at(ReviewRowFamily::Decision, &rows.decisions),
                    second: RowAt {
                        family: ReviewRowFamily::Decision,
                        position: second,
                    },
                });
            }
        }
        let approvals = rows
            .decisions
            .iter()
            .copied()
            .filter(|&position| dispositions[position].decision == DispositionDecision::Accepted)
            .collect::<Vec<_>>();
        if !approvals.is_empty() && !rows.withdrawals.is_empty() {
            faults.push(ReviewRowFault::AcceptedAndWithdrawn {
                submission: submission.clone(),
                approvals: rows_at(ReviewRowFamily::Decision, &approvals),
                withdrawals: rows_at(ReviewRowFamily::Withdrawal, &rows.withdrawals),
            });
        }
        faults
    }
}

fn rows_at(family: ReviewRowFamily, positions: &[usize]) -> Vec<RowAt> {
    positions
        .iter()
        .map(|&position| RowAt { family, position })
        .collect()
}

/// The fault of an ID whose lines do not have equal contents. `second` is
/// the first line whose contents are not the contents of the first line.
fn id_on_lines(
    id: &StableId,
    family: ReviewRowFamily,
    positions: &[usize],
    second: usize,
) -> ReviewRowFault {
    ReviewRowFault::IdOnTwoLines {
        id: id.clone(),
        lines: rows_at(family, positions),
        second: RowAt {
            family,
            position: second,
        },
    }
}
