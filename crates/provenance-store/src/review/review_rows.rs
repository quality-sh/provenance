//! One read of the stored review rows of a scope, and the review state of
//! each record from [`provenance_core::review::read_review_rows`].

use crate::shards;
use crate::state_store::{message_ids::ShardLine, readers::read_jsonl_located, StateStore};
use provenance_core::{
    review::{
        is_review_submission, read_review_rows, RecordReview, ReviewReading, StoredReviewRows,
    },
    DispositionRecord, NodeType, ProposalCard, ScopeId, StableId, Withdrawal,
};
use provenance_macros::rule;
use std::collections::HashMap;

/// The stored review rows of one scope, each with its line, and the
/// feedback Messages of the decisions.
pub(super) struct StoredRows {
    pub(super) proposals: Vec<ProposalCard>,
    pub(super) dispositions: Vec<DispositionRecord>,
    pub(super) withdrawals: Vec<Withdrawal>,
    pub(super) lines: RowLines,
    /// The feedback Message of the first feedback Discussion of each decision.
    feedback: HashMap<String, StableId>,
}

/// The line of each stored row, by family and position.
pub(super) struct RowLines {
    pub(super) proposals: Vec<ShardLine>,
    pub(super) dispositions: Vec<ShardLine>,
    pub(super) withdrawals: Vec<ShardLine>,
}

fn split<T>(located: Vec<(T, ShardLine)>) -> (Vec<T>, Vec<ShardLine>) {
    located.into_iter().unzip()
}

impl StoredRows {
    /// Reads the Proposal, Disposition and Withdrawal files of `scope` and
    /// its feedback Discussions. Review submissions and their decisions are
    /// only in these files: a batch write cannot hold them.
    pub(super) fn read(store: &StateStore, scope: &ScopeId) -> anyhow::Result<Self> {
        let layout = &store.layout;
        let (proposals, proposal_lines) = split(read_jsonl_located(
            store,
            &shards::proposal_cards_path(layout, scope),
        )?);
        // The order of the reads decides which error of a malformed file
        // wins: the Withdrawals, then the Discussions, then the
        // Dispositions.
        let (withdrawals, withdrawal_lines) = split(store.list_withdrawals_located(scope)?);
        let mut feedback = HashMap::new();
        for discussion in store.list_discussions(scope)? {
            if let Some(disposition) = discussion.disposition_id {
                feedback
                    .entry(disposition.as_str().to_owned())
                    .or_insert(discussion.root_message_id);
            }
        }
        let (dispositions, disposition_lines) = split(read_jsonl_located(
            store,
            &shards::dispositions_path(layout, scope),
        )?);
        Ok(Self {
            proposals,
            dispositions,
            withdrawals,
            lines: RowLines {
                proposals: proposal_lines,
                dispositions: disposition_lines,
                withdrawals: withdrawal_lines,
            },
            feedback,
        })
    }

    /// The review state of the scope. Refuses, at its line, a withdrawal that
    /// names no review submission.
    pub(super) fn facts(&self) -> anyhow::Result<CycleFacts<'_>> {
        let reading = read_review_rows(StoredReviewRows {
            proposals: &self.proposals,
            dispositions: &self.dispositions,
            withdrawals: &self.withdrawals,
        });
        if let Some(fault) = reading.scope_fault() {
            return Err(super::review_row_refusal::refusal(self, fault));
        }
        Ok(CycleFacts {
            rows: self,
            reading,
        })
    }
}

/// The review state of one scope, read from its records.
pub(super) struct CycleFacts<'a> {
    rows: &'a StoredRows,
    reading: ReviewReading<'a>,
}

impl CycleFacts<'_> {
    /// The review state of one record. Refuses the record when its review
    /// rows do not agree; the other records read as usual.
    #[rule("rule_review_row_refusal_stays_with_record")]
    pub(super) fn record(&self, kind: NodeType, id: &StableId) -> anyhow::Result<RecordReview<'_>> {
        self.reading
            .record(kind, id)
            .map_err(|fault| super::review_row_refusal::refusal(self.rows, fault))
    }

    /// The review submission with this ID. Refuses with the fault of a
    /// record that a line of the ID names when the review rows of that
    /// record do not agree. `None` when no review submission has the ID.
    pub(super) fn submission(&self, id: &StableId) -> anyhow::Result<Option<&ProposalCard>> {
        self.reading
            .submission(id)
            .map_err(|fault| super::review_row_refusal::refusal(self.rows, fault))
    }

    /// The kind and the ID of the record of each review submission line, in
    /// line order. A submission whose ID is on lines that disagree counts.
    pub(super) fn submitted_records(&self) -> impl Iterator<Item = (NodeType, &StableId)> + '_ {
        self.rows
            .proposals
            .iter()
            .filter(|proposal| is_review_submission(proposal))
            .map(|submission| {
                let target = &submission.traceability.target;
                (NodeType::from(target.artifact_type), &target.artifact_id)
            })
    }

    /// Whether a Proposal is a review submission.
    pub(super) fn is_submission(&self, proposal: &StableId) -> bool {
        self.reading.is_submission(proposal)
    }

    /// Refuses a review submission that a write addresses through a record
    /// that no line of the submission names. The lines of the submission
    /// answer this, so the refusal does not depend on the other review rows
    /// of its record. A Proposal that is not a review submission passes.
    pub(super) fn ensure_addressed(
        &self,
        proposal: &StableId,
        addressed: Option<(NodeType, &StableId)>,
    ) -> anyhow::Result<()> {
        let belongs = addressed.is_none_or(|(kind, record)| {
            !self.is_submission(proposal) || self.reading.submission_names(proposal, kind, record)
        });
        crate::write_error::ensure!(
            InvalidUpdate,
            belongs,
            "the review submission does not belong to the addressed record"
        );
        Ok(())
    }

    /// The feedback Message a decision published, if it did.
    pub(super) fn feedback_for(&self, disposition: &StableId) -> Option<StableId> {
        self.rows.feedback.get(disposition.as_str()).cloned()
    }

    /// The result of the decisions on one submission when they agree.
    pub(super) fn agreed_decision(
        &self,
        submission: &StableId,
    ) -> Option<provenance_core::DispositionDecision> {
        self.reading.agreed_decision(submission)
    }

    /// The submission of this record that still waits for a decision, if any.
    pub(super) fn pending_submission(
        &self,
        store: &StateStore,
        scope: &ScopeId,
        kind: NodeType,
        record_id: &StableId,
    ) -> anyhow::Result<Option<ProposalCard>> {
        let record = crate::families::review_families::record(store, scope, kind, record_id)?;
        let current_revision = super::save::current_revision(&record)?;
        Ok(self
            .record(kind, record_id)?
            .pending_at(current_revision.as_ref())
            .cloned())
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
        let record = crate::families::review_families::record(store, scope, kind, record_id)?;
        let current_revision = super::save::current_revision(&record)?
            .ok_or_else(|| crate::refusal::fault!("the submitted record has no review revision"))?;
        let current_submission = self
            .pending_submission(store, scope, kind, record_id)?
            .map(|proposal| proposal.id);
        Ok(crate::write_error::WriteFailure::ReviewSubmissionConflict {
            current_submission,
            current_revision,
        })
    }
}

impl StateStore {
    /// Whether a stored line of `scope` holds a withdrawal or a review
    /// submission, also when the lines of its ID do not agree.
    pub(super) fn has_review_rows(&self, scope: &ScopeId) -> anyhow::Result<bool> {
        if !self.list_withdrawals_located(scope)?.is_empty() {
            return Ok(true);
        }
        let proposals: Vec<(ProposalCard, ShardLine)> =
            read_jsonl_located(self, &shards::proposal_cards_path(&self.layout, scope))?;
        Ok(proposals
            .iter()
            .any(|(proposal, _)| is_review_submission(proposal)))
    }

    /// The review submission with this ID, as [`CycleFacts::submission`]
    /// gives it, from one read of the review rows of `scope`.
    pub(crate) fn stored_review_submission(
        &self,
        scope: &ScopeId,
        id: &StableId,
    ) -> anyhow::Result<Option<ProposalCard>> {
        let stored = StoredRows::read(self, scope)?;
        let facts = stored.facts()?;
        Ok(facts.submission(id)?.cloned())
    }

    /// Each fault of the stored review rows of `scope`, as the text of its
    /// refusal: the file, the line and the message that a read of the
    /// record gives. A withdrawal that names no review submission is one of
    /// them.
    #[rule("rule_check_reports_review_row_faults")]
    pub fn review_row_faults(&self, scope: &ScopeId) -> anyhow::Result<Vec<String>> {
        let stored = StoredRows::read(self, scope)?;
        let reading = read_review_rows(StoredReviewRows {
            proposals: &stored.proposals,
            dispositions: &stored.dispositions,
            withdrawals: &stored.withdrawals,
        });
        Ok(reading
            .faults()
            .map(|fault| super::review_row_refusal::refusal(&stored, fault).to_string())
            .collect())
    }
}
