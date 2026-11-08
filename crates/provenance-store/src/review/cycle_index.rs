//! Lookups over one read of a scope's review records.
//!
//! [`CycleIndex::new`] reads the Proposal, Disposition, Withdrawal and
//! feedback lists once. Each lookup then costs one hash probe instead of a
//! scan of a list. Where a list holds two entries with one id, the index
//! keeps the first, as a scan with `find` does. The lists of one record keep
//! the stored order.

use provenance_core::{DispositionRecord, NodeType, ProposalCard, StableId, Withdrawal};
use std::collections::{HashMap, HashSet};

/// The review submissions and the Dispositions of one record, as positions
/// in the scope lists, in stored order.
#[derive(Default)]
struct RecordEntries {
    submissions: Vec<usize>,
    dispositions: Vec<usize>,
}

pub(super) struct CycleIndex {
    /// The position of the first Proposal with each id.
    proposals: HashMap<String, usize>,
    /// The review cycle of the first submission with each id, if its key names one.
    cycles: HashMap<String, Option<u64>>,
    decided: HashSet<String>,
    withdrawn: HashSet<String>,
    /// The feedback Message of the first feedback Discussion of each Disposition.
    feedback: HashMap<String, StableId>,
    /// The entries of each record, by record kind and then by record id.
    records: HashMap<&'static str, HashMap<String, RecordEntries>>,
}

fn target(proposal: &ProposalCard) -> (&'static str, &str) {
    let target = &proposal.traceability.target;
    (
        NodeType::from(target.artifact_type).as_str(),
        target.artifact_id.as_str(),
    )
}

impl CycleIndex {
    pub(super) fn new(
        proposals: &[ProposalCard],
        submissions: &[ProposalCard],
        dispositions: &[DispositionRecord],
        withdrawals: &[Withdrawal],
        feedback: &[(StableId, StableId)],
    ) -> Self {
        let mut index = Self {
            proposals: HashMap::with_capacity(proposals.len()),
            cycles: HashMap::with_capacity(submissions.len()),
            decided: dispositions
                .iter()
                .map(|disposition| disposition.proposal_id.as_str().to_owned())
                .collect(),
            withdrawn: withdrawals
                .iter()
                .map(|withdrawal| withdrawal.proposal_id.as_str().to_owned())
                .collect(),
            feedback: HashMap::with_capacity(feedback.len()),
            records: HashMap::new(),
        };
        for (position, proposal) in proposals.iter().enumerate() {
            index
                .proposals
                .entry(proposal.id.as_str().to_owned())
                .or_insert(position);
        }
        for (position, submission) in submissions.iter().enumerate() {
            index
                .cycles
                .entry(submission.id.as_str().to_owned())
                .or_insert_with(|| super::decision_state::cycle_of(submission));
            index.entry(target(submission)).submissions.push(position);
        }
        for (position, disposition) in dispositions.iter().enumerate() {
            if let Some(&proposal) = index.proposals.get(disposition.proposal_id.as_str()) {
                index
                    .entry(target(&proposals[proposal]))
                    .dispositions
                    .push(position);
            }
        }
        for (disposition, message) in feedback {
            index
                .feedback
                .entry(disposition.as_str().to_owned())
                .or_insert_with(|| message.clone());
        }
        index
    }

    fn entry(&mut self, (kind, id): (&'static str, &str)) -> &mut RecordEntries {
        self.records
            .entry(kind)
            .or_default()
            .entry(id.to_owned())
            .or_default()
    }

    fn of(&self, kind: NodeType, record_id: &StableId) -> Option<&RecordEntries> {
        self.records.get(kind.as_str())?.get(record_id.as_str())
    }

    /// The position of the first Proposal with this id.
    pub(super) fn proposal(&self, proposal: &StableId) -> Option<usize> {
        self.proposals.get(proposal.as_str()).copied()
    }

    /// Whether a Proposal with this id is a review submission.
    pub(super) fn is_submission(&self, proposal: &StableId) -> bool {
        self.cycles.contains_key(proposal.as_str())
    }

    /// The review cycle of a submission, or `None` for a submission whose
    /// key names no cycle and for an id that names no submission.
    pub(super) fn cycle(&self, proposal: &StableId) -> Option<u64> {
        self.cycles.get(proposal.as_str()).copied().flatten()
    }

    pub(super) fn is_decided(&self, proposal: &StableId) -> bool {
        self.decided.contains(proposal.as_str())
    }

    pub(super) fn is_withdrawn(&self, proposal: &StableId) -> bool {
        self.withdrawn.contains(proposal.as_str())
    }

    pub(super) fn feedback_for(&self, disposition: &StableId) -> Option<StableId> {
        self.feedback.get(disposition.as_str()).cloned()
    }

    /// The positions of the record's review submissions, in stored order.
    pub(super) fn submissions_of(&self, kind: NodeType, record_id: &StableId) -> &[usize] {
        self.of(kind, record_id)
            .map_or(&[], |entries| entries.submissions.as_slice())
    }

    /// The positions of the Dispositions whose Proposal names the record, in
    /// stored order.
    pub(super) fn dispositions_of(&self, kind: NodeType, record_id: &StableId) -> &[usize] {
        self.of(kind, record_id)
            .map_or(&[], |entries| entries.dispositions.as_slice())
    }
}
