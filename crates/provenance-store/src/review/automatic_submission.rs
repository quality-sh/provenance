//! Automatic review submissions for enrolled graph record writes.

use super::{decision_state::check_request_size, guard, SubmitRecordReview};
use crate::{shards, state_store::StateStore};
use provenance_core::review::ReviewRecord;
use provenance_macros::rule;

impl StateStore {
    /// Binds a server-created review submission to a new content revision in the
    /// same write, unless that revision is already pending or accepted.
    #[rule("rule_content_change_opens_review_submission")]
    pub(super) fn commit_automatic_submission<T>(
        &self,
        record: &T,
        actor: &str,
        revision: &provenance_core::StableId,
    ) -> anyhow::Result<()>
    where
        T: Clone + Into<ReviewRecord>,
    {
        let record: ReviewRecord = record.clone().into();
        let state = self.record_decision_state(record.scope_id(), record.kind(), record.id())?;
        let pending = state
            .pending
            .is_some_and(|pending| pending.revision == *revision);
        let accepted = state
            .current_acceptance
            .is_some_and(|decision| decision.revision.as_ref() == Some(revision));
        if pending || accepted {
            return Ok(());
        }
        let value = serde_json::to_value(&record)?;
        let summary = ["statement", "position", "question", "title", "name"]
            .into_iter()
            .find_map(|field| value.get(field).and_then(serde_json::Value::as_str))
            .unwrap_or_else(|| record.id().as_str())
            .to_owned();
        let input = SubmitRecordReview {
            scope_id: record.scope_id().clone(),
            actor: actor.to_owned(),
            record_kind: record.kind(),
            record_id: record.id().clone(),
            declared_by: value
                .get("declared_by")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned),
            title: format!("Review {}", record.id().as_str()),
            summary,
            confidence: None,
            source_ids: Vec::new(),
            evidence_references: Vec::new(),
            builds_on: Vec::new(),
            expected_revision: Some(revision.clone()),
            revises: None,
        };
        check_request_size(&input)?;
        guard::with_writer(
            &shards::proposal_cards_path(&self.layout, record.scope_id()),
            "*",
            || self.commit_submission(input),
        )?;
        Ok(())
    }
}
