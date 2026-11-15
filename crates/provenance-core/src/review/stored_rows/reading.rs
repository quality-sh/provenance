//! The reading of the stored review rows: which rows read as one row, and
//! which rows do not agree.

use super::row_groups::{group_by_id, IdGroup};
use super::{
    is_review_submission, review_cycle, submission_order, target, RecordKey, RecordRows,
    ReviewReading, ReviewRowFamily, ReviewRowFault, RowAt, SubmissionLines, SubmissionRows,
};
use crate::{DispositionDecision, StableId};
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
                self.disputed.push((first, SubmissionRows::default()));
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
                let family = ReviewRowFamily::Decision;
                let fault = id_on_lines(&dispositions[first].id, family, &group.positions, second);
                self.fault(fault, &records);
                continue;
            }
            // A decision on a submission whose ID is on lines that disagree
            // is a part of the fault of that submission.
            let submission = dispositions[first].proposal_id.as_str();
            if let Some((_, rows)) = self.submissions.get_mut(submission) {
                rows.decisions.push(first);
                kept.push(first);
            } else if let Some(rows) = self.disputed_rows(submission) {
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
                let family = ReviewRowFamily::Withdrawal;
                let fault = id_on_lines(&withdrawals[first].id, family, &group.positions, second);
                self.fault(fault, &records);
                continue;
            }
            let withdrawal = &withdrawals[first];
            if !self.is_submission(&withdrawal.proposal_id) {
                kept.push(first);
                let fault = ReviewRowFault::NoSubmission {
                    proposal: withdrawal.proposal_id.clone(),
                    lines: rows_at(ReviewRowFamily::Withdrawal, &group.positions),
                };
                self.fault(fault, &[]);
            } else if let Some((_, rows)) =
                self.submissions.get_mut(withdrawal.proposal_id.as_str())
            {
                rows.withdrawals.push(first);
                kept.push(first);
            } else if let Some(rows) = self.disputed_rows(withdrawal.proposal_id.as_str()) {
                // A withdrawal of a submission whose ID is on lines that
                // disagree is a part of the fault of that submission.
                rows.withdrawals.push(first);
            }
        }
        kept.sort_unstable();
        self.once.withdrawals = kept;
    }

    fn disputed_rows(&mut self, submission: &str) -> Option<&mut SubmissionRows> {
        self.disputed
            .iter_mut()
            .find(|(id, _)| *id == submission)
            .map(|(_, rows)| rows)
    }

    /// Gives the faults of the decisions and the withdrawals of each
    /// submission whose ID is on lines that disagree to each record that a
    /// line of the ID names, so that one check reports each fault.
    pub(super) fn read_disputed_submissions(&mut self) {
        let proposals = self.rows.proposals;
        let disputed = std::mem::take(&mut self.disputed);
        for (id, rows) in &disputed {
            let records = self.records_of_submission(id);
            let submission = &proposals[self.submission_ids[id][0]].id;
            for fault in self.submission_faults(submission, rows) {
                self.fault(fault, &records);
            }
        }
        self.disputed = disputed;
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
            let mut in_line_order = self.records[key.0][key.1].submissions.clone();
            in_line_order.sort_unstable();
            let mut faults = self.shared_cycles(&in_line_order);
            for &position in &in_line_order {
                let id = &proposals[position].id;
                let (_, rows) = &self.submissions[id.as_str()];
                faults.extend(self.submission_faults(id, rows));
            }
            for fault in faults {
                self.fault(fault, &[key]);
            }
        }
    }

    /// The lines of each decision and of each withdrawal that name the
    /// submission, by family, then in line order.
    fn lines_naming(&self, submission: &StableId) -> Vec<RowAt> {
        let decisions = self.rows.dispositions.iter().map(|row| &row.proposal_id);
        let withdrawals = self.rows.withdrawals.iter().map(|row| &row.proposal_id);
        let mut lines = naming(ReviewRowFamily::Decision, decisions, submission);
        lines.extend(naming(ReviewRowFamily::Withdrawal, withdrawals, submission));
        lines
    }

    /// One fault for each review cycle that more than one of the
    /// submissions has, in the line order of the second submission.
    /// `in_line_order` are the submissions of one record.
    fn shared_cycles(&self, in_line_order: &[usize]) -> Vec<ReviewRowFault> {
        let proposals = self.rows.proposals;
        let mut cycles: Vec<(u64, Vec<usize>)> = Vec::new();
        for &position in in_line_order {
            let Some(cycle) = review_cycle(&proposals[position]) else {
                continue;
            };
            match cycles.iter_mut().find(|(known, _)| *known == cycle) {
                Some((_, positions)) => positions.push(position),
                None => cycles.push((cycle, vec![position])),
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
    /// `rows` are the decisions and the withdrawals that name it.
    fn submission_faults(
        &self,
        submission: &StableId,
        rows: &SubmissionRows,
    ) -> Vec<ReviewRowFault> {
        let dispositions = self.rows.dispositions;
        let lines = self.lines_naming(submission);
        let of_family = |family| {
            lines
                .iter()
                .copied()
                .filter(move |at: &RowAt| at.family == family)
        };
        let mut faults = Vec::new();
        if let Some((&first, rest)) = rows.decisions.split_first() {
            let result = dispositions[first].decision;
            if let Some(&second) = rest
                .iter()
                .find(|&&position| dispositions[position].decision != result)
            {
                faults.push(ReviewRowFault::DifferentResults {
                    submission: submission.clone(),
                    decisions: of_family(ReviewRowFamily::Decision).collect(),
                    second: RowAt {
                        family: ReviewRowFamily::Decision,
                        position: second,
                    },
                });
            }
        }
        let approves =
            |position: usize| dispositions[position].decision == DispositionDecision::Accepted;
        if rows.decisions.iter().any(|&position| approves(position)) && !rows.withdrawals.is_empty()
        {
            faults.push(ReviewRowFault::AcceptedAndWithdrawn {
                submission: submission.clone(),
                approvals: of_family(ReviewRowFamily::Decision)
                    .filter(|at| approves(at.position))
                    .collect(),
                withdrawals: of_family(ReviewRowFamily::Withdrawal).collect(),
            });
        }
        faults
    }
}

/// The rows of `family` whose submission is `submission`. `named` gives the
/// submission of each row, in line order.
fn naming<'a>(
    family: ReviewRowFamily,
    named: impl Iterator<Item = &'a StableId>,
    submission: &StableId,
) -> Vec<RowAt> {
    named
        .enumerate()
        .filter(|(_, row_submission)| *row_submission == submission)
        .map(|(position, _)| RowAt { family, position })
        .collect()
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
