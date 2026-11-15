//! Current and historical decision reads for one record.
//!
//! The current read joins the bound submissions, their dispositions and
//! withdrawals, the feedback Discussions, and the record's current review
//! revision. Legacy unbound
//! dispositions keep their place in the history and stay authoritative for
//! their own proposals; none of them attests the record's current contents.

use super::review_rows::{CycleFacts, StoredRows};
use crate::{families::review_families::ReviewFamilyIndex, state_store::StateStore};
use provenance_core::{
    protocol::DocumentReviewSummary,
    review::{PendingSubmission, RecordedDecision, RequirementDecisionState, ReviewRecord},
    DispositionDecision, NodeType, Resolution, ResolutionStatus, ScopeId, StableId,
};
use provenance_macros::rule;
use std::cell::OnceCell;
use std::collections::HashMap;

pub struct DocumentReviewState {
    pub records: Vec<(NodeType, StableId, DocumentReviewSummary)>,
    /// The records whose review rejected them before any acceptance.
    pub rejected_new: Vec<(NodeType, StableId)>,
    pub digest: String,
    /// The position of the first entry of each record in `records`, by
    /// record kind and then by record id.
    positions: HashMap<&'static str, HashMap<String, usize>>,
}

impl DocumentReviewState {
    fn new(
        records: Vec<(NodeType, StableId, DocumentReviewSummary)>,
        rejected_new: Vec<(NodeType, StableId)>,
        digest: String,
    ) -> Self {
        let mut positions = HashMap::<_, HashMap<_, _>>::new();
        for (position, (kind, id, _)) in records.iter().enumerate() {
            positions
                .entry(kind.as_str())
                .or_default()
                .entry(id.as_str().to_owned())
                .or_insert(position);
        }
        Self {
            records,
            rejected_new,
            digest,
            positions,
        }
    }

    pub fn summary(&self, kind: NodeType, id: &StableId) -> DocumentReviewSummary {
        self.positions
            .get(kind.as_str())
            .and_then(|ids| ids.get(id.as_str()))
            .map_or_default(|&position| self.records[position].2.clone())
    }
}

/// A record that no review accepted and whose latest review decision rejected
/// it is a rejected new record. A rejected change to an accepted record is
/// not. Only decisions on review submissions count: a decision on a
/// proposal of another type that names the record, such as a candidate
/// answer to a Question, decides nothing about the record.
#[rule("rule_review_document_hides_rejected_new_records")]
fn rejected_before_acceptance(state: &RequirementDecisionState, facts: &CycleFacts<'_>) -> bool {
    let reviews = state
        .decisions
        .iter()
        .filter(|decision| facts.is_submission(&decision.disposition.proposal_id))
        .collect::<Vec<_>>();
    state.pending.is_none()
        && reviews
            .last()
            .is_some_and(|decision| decision.disposition.decision == DispositionDecision::Rejected)
        && !reviews
            .iter()
            .any(|decision| decision.disposition.decision == DispositionDecision::Accepted)
}

impl StateStore {
    /// Builds the current review outcome for each entry and the totals for the document.
    /// Refuses the whole document when the review rows of one of its records do not agree.
    #[rule("rule_document_entry_has_review_outcome")]
    #[rule("rule_document_has_review_totals")]
    #[rule("rule_document_read_refuses_with_record")]
    pub(crate) fn document_review_state(
        &self,
        scope: &ScopeId,
        records: &[(NodeType, StableId)],
    ) -> anyhow::Result<DocumentReviewState> {
        self.with_repository_publication(|held| {
            let stored = StoredRows::read(held, scope)?;
            let facts = stored.facts()?;
            let mut families = ReviewFamilyIndex::new(held, scope, records);
            let mut states = Vec::new();
            for (kind, id) in records {
                // A reference to a record that no longer exists has no review state.
                if let Some(record) = families.find(*kind, id)? {
                    states.push(Self::record_decision_state_from_parts(record, &facts)?);
                }
            }
            let digest = crate::canonical_digest::digest(
                &crate::canonical_digest::canonical_bytes(&states)?,
            );
            let rejected_new = states
                .iter()
                .filter(|state| rejected_before_acceptance(state, &facts))
                .map(|state| (state.record_kind, state.record_id.clone()))
                .collect();
            let records = states
                .iter()
                .map(|state| {
                    (
                        state.record_kind,
                        state.record_id.clone(),
                        DocumentReviewSummary::from(state),
                    )
                })
                .collect::<Vec<_>>();
            Ok(DocumentReviewState::new(records, rejected_new, digest))
        })
    }

    /// Reads the decision state of one Requirement: the submission still
    /// waiting, the acceptance that matches current content, every terminal
    /// decision, and every withdrawal.
    pub fn requirement_decision_state(
        &self,
        scope: &ScopeId,
        requirement_id: &StableId,
    ) -> anyhow::Result<RequirementDecisionState> {
        self.record_decision_state(scope, NodeType::Requirement, requirement_id)
    }

    pub fn record_decision_state(
        &self,
        scope: &ScopeId,
        kind: NodeType,
        record_id: &StableId,
    ) -> anyhow::Result<RequirementDecisionState> {
        let record = crate::families::review_families::record(self, scope, kind, record_id)?;
        self.record_decision_state_for_record(&record)
    }

    pub(super) fn record_decision_state_for_record(
        &self,
        record: &ReviewRecord,
    ) -> anyhow::Result<RequirementDecisionState> {
        let stored = StoredRows::read(self, record.scope_id())?;
        Self::record_decision_state_from_parts(record, &stored.facts()?)
    }

    /// The decision state of one record from one read of the scope's
    /// review records. Refuses the record when its review rows do not
    /// agree.
    fn record_decision_state_from_parts(
        record: &ReviewRecord,
        facts: &CycleFacts<'_>,
    ) -> anyhow::Result<RequirementDecisionState> {
        let kind = record.kind();
        let record_id = record.id();
        let current_revision = super::save::current_revision(record)?;
        let review = facts.record(kind, record_id)?;
        let pending = review
            .pending_at(current_revision.as_ref())
            .cloned()
            .map(|proposal| PendingSubmission {
                proposal_id: proposal.id,
                revision: proposal
                    .record_revision
                    .expect("pending submissions carry bindings")
                    .revision,
                actor: proposal.actor.unwrap_or_default(),
                revises: proposal.revises,
            });
        let recorded: Vec<RecordedDecision> = review
            .decisions()
            .into_iter()
            .map(|(disposition, proposal)| RecordedDecision {
                revision: proposal
                    .record_revision
                    .as_ref()
                    .map(|binding| binding.revision.clone()),
                feedback_message_id: facts.feedback_for(&disposition.id),
                disposition: disposition.clone(),
            })
            .collect();
        let current_acceptance = recorded
            .iter()
            .filter(|decision| decision.disposition.decision == DispositionDecision::Accepted)
            .find(|decision| {
                decision.revision.is_some()
                    && decision.revision.as_ref() == current_revision.as_ref()
            })
            .cloned();
        Ok(RequirementDecisionState {
            record_kind: kind,
            record_id: record_id.clone(),
            current_revision,
            pending,
            current_acceptance,
            decisions: recorded,
            withdrawn: review.withdrawn(),
        })
    }
}

/// Whether a reviewer accepted the current revision of a Resolution of one
/// scope, for one citation check. The first question reads the review rows
/// of the scope, and each later question of the check uses that reading, so
/// a citation check that needs no acceptance reads no review rows.
pub struct ResolutionAcceptance<'r> {
    store: &'r StateStore,
    scope: &'r ScopeId,
    rows: &'r OnceCell<StoredRows>,
    facts: Option<CycleFacts<'r>>,
}

impl ResolutionAcceptance<'_> {
    /// Gives `check` the acceptance of the Resolutions of `scope`. The
    /// reading of the review rows lasts for the call of `check` only.
    pub fn for_one_check<T>(
        store: &StateStore,
        scope: &ScopeId,
        check: impl FnOnce(&mut ResolutionAcceptance<'_>) -> T,
    ) -> T {
        let rows = OnceCell::new();
        check(&mut ResolutionAcceptance {
            store,
            scope,
            rows: &rows,
            facts: None,
        })
    }
}

impl<'r> ResolutionAcceptance<'r> {
    /// The review state of the scope, read at the first question.
    fn facts(&mut self) -> anyhow::Result<&CycleFacts<'r>> {
        let facts = match self.facts.take() {
            Some(facts) => facts,
            None => {
                let cell: &'r OnceCell<StoredRows> = self.rows;
                let rows = match cell.get() {
                    Some(rows) => rows,
                    None => {
                        let read = StoredRows::read(self.store, self.scope)?;
                        cell.get_or_init(|| read)
                    }
                };
                rows.facts()?
            }
        };
        Ok(self.facts.insert(facts))
    }

    /// Whether `resolution` is a draft, review or proposed Resolution whose
    /// current revision a reviewer accepted. A citation check reads it as
    /// approved, because an approval does not change the lifecycle status.
    /// Refuses with the refusal of a read of `resolution` when its review
    /// rows do not agree.
    #[rule("rule_review_acceptance_approves_resolution_citation")]
    pub fn accepts(&mut self, resolution: &Resolution) -> anyhow::Result<bool> {
        if !matches!(
            resolution.status,
            ResolutionStatus::Draft | ResolutionStatus::Review | ResolutionStatus::Proposed
        ) {
            return Ok(false);
        }
        let record = ReviewRecord::Resolution(resolution.clone());
        let state = StateStore::record_decision_state_from_parts(&record, self.facts()?)?;
        Ok(state.current_acceptance.is_some())
    }
}
