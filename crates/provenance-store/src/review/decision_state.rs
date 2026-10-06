//! Shared decision-cycle helpers: the review state read from Proposal,
//! Disposition, Withdrawal and Discussion records, and the checks a
//! submission or a decision must pass against the scope.

use crate::state_store::StateStore;
use provenance_core::{
    review::{CycleEntry, CycleFact},
    DispositionDecision, DispositionRecord, NodeType, ProposalCard, ProposalType, ScopeId,
    StableId, Withdrawal,
};
use provenance_macros::rule;

const MAX_SAFE_CYCLE: u64 = (1 << 53) - 1;

pub(super) fn check_request_size(input: &impl serde::Serialize) -> anyhow::Result<()> {
    anyhow::ensure!(
        serde_json::to_vec(input)?.len() <= 1_048_576,
        "review request exceeds the operation byte budget"
    );
    Ok(())
}

/// The key of the review Proposal for one record and one review cycle.
#[rule("rule_review_proposal_key_names_record_cycle")]
pub(super) fn review_proposal_key(kind: NodeType, record_id: &StableId, cycle: u64) -> String {
    format!("review:{}:{}:{cycle}", kind.as_str(), record_id.as_str())
}

/// The response of one decision-cycle write.
pub(super) struct Receipt {
    pub(super) id: StableId,
    pub(super) record_kind: NodeType,
    pub(super) record_id: StableId,
    pub(super) proposal: ProposalCard,
    pub(super) fact: CycleFact,
    pub(super) disposition_id: Option<StableId>,
    pub(super) feedback_message_id: Option<StableId>,
    pub(super) actor: String,
}

impl Receipt {
    /// Builds the response of a decision-cycle write.
    pub(super) fn entry(self) -> CycleEntry {
        CycleEntry {
            scope_id: self.proposal.scope_id.clone(),
            id: self.id,
            sequence: cycle_of(&self.proposal).unwrap_or_default(),
            proposal_key: (self.fact == CycleFact::Submitted)
                .then(|| self.proposal.proposal_key.clone()),
            record_kind: self.record_kind,
            record_id: self.record_id,
            proposal_id: self.proposal.id,
            fact: self.fact,
            disposition_id: self.disposition_id,
            feedback_message_id: self.feedback_message_id,
            actor: self.actor,
        }
    }
}

/// The review cycle a Proposal key names, or `None` for a key in another
/// form, such as the random key of an older submission.
pub(super) fn cycle_of(proposal: &ProposalCard) -> Option<u64> {
    let kind = NodeType::from(proposal.traceability.target.artifact_type);
    let prefix = format!(
        "review:{}:{}:",
        kind.as_str(),
        proposal.traceability.target.artifact_id.as_str()
    );
    proposal
        .proposal_key
        .strip_prefix(&prefix)
        .and_then(|cycle| cycle.parse().ok())
}

/// The review state of one scope, read from its records.
pub(super) struct CycleFacts {
    submissions: Vec<ProposalCard>,
    dispositions: Vec<DispositionRecord>,
    withdrawals: Vec<Withdrawal>,
    feedback: Vec<(StableId, StableId)>,
}

impl CycleFacts {
    /// Reads the scope's review submissions, Dispositions, Withdrawals and
    /// feedback Discussions, and refuses a Withdrawal that names no review
    /// submission.
    pub(super) fn validated(store: &StateStore, scope: &ScopeId) -> anyhow::Result<Self> {
        let submissions = store
            .list_proposal_definitions(scope)?
            .into_iter()
            .filter(|proposal| proposal.proposal_type == ProposalType::RecordRevision)
            .collect::<Vec<_>>();
        let withdrawals = store.list_withdrawals(scope)?;
        let mut withdrawn = std::collections::BTreeSet::new();
        for withdrawal in &withdrawals {
            anyhow::ensure!(
                submissions
                    .iter()
                    .any(|proposal| proposal.id == withdrawal.proposal_id),
                "withdrawal {} names no review submission",
                withdrawal.id.as_str()
            );
            anyhow::ensure!(
                withdrawn.insert(withdrawal.proposal_id.as_str()),
                "review submission {} is withdrawn twice",
                withdrawal.proposal_id.as_str()
            );
        }
        let feedback = store
            .list_discussions(scope)?
            .into_iter()
            .filter_map(|discussion| {
                discussion
                    .disposition_id
                    .map(|disposition| (disposition, discussion.root_message_id))
            })
            .collect();
        Ok(Self {
            submissions,
            dispositions: store.list_dispositions(scope)?,
            withdrawals,
            feedback,
        })
    }

    pub(super) fn is_withdrawn(&self, proposal: &StableId) -> bool {
        self.withdrawals
            .iter()
            .any(|withdrawal| withdrawal.proposal_id == *proposal)
    }

    pub(super) fn is_decided(&self, proposal: &StableId) -> bool {
        self.dispositions
            .iter()
            .any(|disposition| disposition.proposal_id == *proposal)
    }

    /// The feedback Message a decision published, if it did.
    pub(super) fn feedback_for(&self, disposition: &StableId) -> Option<StableId> {
        self.feedback
            .iter()
            .find(|(decided, _)| decided == disposition)
            .map(|(_, message)| message.clone())
    }

    /// The order of one review submission: its review cycle, then its id.
    /// A submission whose key names no cycle keeps the end of the order.
    pub(super) fn order_of(&self, proposal: &StableId) -> (u64, String) {
        let cycle = self
            .submissions
            .iter()
            .find(|submission| submission.id == *proposal)
            .and_then(cycle_of)
            .unwrap_or(u64::MAX);
        (cycle, proposal.as_str().to_owned())
    }

    pub(super) fn has_submissions(&self, kind: NodeType, record_id: &StableId) -> bool {
        self.of_record(kind, record_id).next().is_some()
    }

    fn of_record<'a>(
        &'a self,
        kind: NodeType,
        record_id: &'a StableId,
    ) -> impl Iterator<Item = &'a ProposalCard> + 'a {
        self.submissions.iter().filter(move |proposal| {
            NodeType::from(proposal.traceability.target.artifact_type) == kind
                && proposal.traceability.target.artifact_id == *record_id
        })
    }

    /// The withdrawn submissions of one record, in submission order.
    pub(super) fn withdrawn_submissions(
        &self,
        kind: NodeType,
        record_id: &StableId,
    ) -> Vec<StableId> {
        let mut withdrawn = self
            .of_record(kind, record_id)
            .filter(|proposal| self.is_withdrawn(&proposal.id))
            .map(|proposal| proposal.id.clone())
            .collect::<Vec<_>>();
        withdrawn.sort_by_key(|proposal| self.order_of(proposal));
        withdrawn
    }

    /// The review cycle of the next submission of one record: one more than
    /// the number of review submissions of the record.
    pub(super) fn next_cycle(&self, kind: NodeType, record_id: &StableId) -> anyhow::Result<u64> {
        let next = u64::try_from(self.of_record(kind, record_id).count())?
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("review cycle overflow"))?;
        anyhow::ensure!(
            next <= MAX_SAFE_CYCLE,
            "review cycle exceeds the safe integer limit"
        );
        Ok(next)
    }

    /// The submission of this record that still waits for a decision, if any.
    pub(super) fn pending_submission(
        &self,
        store: &StateStore,
        scope: &ScopeId,
        kind: NodeType,
        record_id: &StableId,
    ) -> anyhow::Result<Option<ProposalCard>> {
        let record = crate::cache::review_families::record(store, scope, kind, record_id)?;
        let current_revision = super::save::current_revision(&record)?;
        Ok(self.pending_submission_at_revision(kind, record_id, current_revision.as_ref()))
    }

    /// The undecided, unwithdrawn submission bound to the current revision.
    pub(super) fn pending_submission_at_revision(
        &self,
        kind: NodeType,
        record_id: &StableId,
        current_revision: Option<&StableId>,
    ) -> Option<ProposalCard> {
        self.of_record(kind, record_id)
            .filter(|proposal| !self.is_decided(&proposal.id) && !self.is_withdrawn(&proposal.id))
            .filter(|proposal| {
                proposal
                    .record_revision
                    .as_ref()
                    .is_some_and(|binding| Some(&binding.revision) == current_revision)
            })
            .max_by_key(|proposal| self.order_of(&proposal.id))
            .cloned()
    }

    /// Returns the current review state in the typed conflict without merging values.
    #[rule("rule_review_conflict_returns_current_value")]
    #[rule("rule_review_conflict_not_merged")]
    pub(super) fn conflict_failure(
        &self,
        store: &StateStore,
        scope: &ScopeId,
        kind: NodeType,
        record_id: &StableId,
    ) -> anyhow::Result<crate::write_error::WriteFailure> {
        let record = crate::cache::review_families::record(store, scope, kind, record_id)?;
        let current_revision = super::save::current_revision(&record)?
            .ok_or_else(|| anyhow::anyhow!("the submitted record has no review revision"))?;
        let current_submission = self
            .pending_submission(store, scope, kind, record_id)?
            .map(|proposal| proposal.id);
        Ok(crate::write_error::WriteFailure::ReviewSubmissionConflict {
            current_submission,
            current_revision,
        })
    }
}

/// The proposal must be a bound record-review submission. Other proposals
/// keep the ordinary disposition paths.
pub(super) fn review_submission(
    store: &StateStore,
    scope: &ScopeId,
    proposal_id: &StableId,
) -> anyhow::Result<ProposalCard> {
    let proposal = store
        .list_proposal_definitions(scope)?
        .into_iter()
        .find(|p| p.id == *proposal_id)
        .ok_or_else(|| anyhow::anyhow!("review submission does not exist"))?;
    anyhow::ensure!(
        proposal.proposal_type == ProposalType::RecordRevision && proposal.record_revision.is_some(),
        "decision-cycle operations address record_revision submissions; other proposals keep their own disposition paths"
    );
    Ok(proposal)
}

/// A resubmission must answer exactly one rejection of one rejected
/// predecessor submission of the same record, and the rejection disposition it
/// answers is recorded on the fresh proposal.
pub(super) fn validated_resubmission(
    store: &StateStore,
    scope: &ScopeId,
    predecessor: &StableId,
    kind: NodeType,
    record_id: &StableId,
) -> anyhow::Result<StableId> {
    let proposals = store.list_proposal_definitions(scope)?;
    let predecessor = proposals
        .iter()
        .find(|p| p.id == *predecessor)
        .ok_or_else(|| anyhow::anyhow!("revises names no proposal"))?;
    anyhow::ensure!(
        predecessor.proposal_type == ProposalType::RecordRevision,
        "a resubmission revises a review submission"
    );
    anyhow::ensure!(
        NodeType::from(predecessor.traceability.target.artifact_type) == kind
            && predecessor.traceability.target.artifact_id == *record_id,
        "a resubmission revises a submission of the same record"
    );
    let rejections: Vec<_> = store
        .list_dispositions(scope)?
        .into_iter()
        .filter(|d| d.proposal_id == predecessor.id)
        .collect();
    anyhow::ensure!(
        rejections.len() == 1,
        "a resubmission answers exactly one disposition of its predecessor"
    );
    anyhow::ensure!(
        rejections[0].decision == DispositionDecision::Rejected,
        "a resubmission answers a rejection"
    );
    Ok(rejections[0].id.clone())
}
