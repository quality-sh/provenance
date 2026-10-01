//! Automatic review submissions for enrolled graph record writes.

use super::{decision_state::request_digest, guard, journal, SubmitRecordReview};
use crate::{shards, state_store::StateStore};
use provenance_core::review::{ReviewEntry, ReviewRecord};

impl StateStore {
    /// Binds a server-created review submission to a new content revision.
    pub(super) fn commit_automatic_submission<T>(
        &self,
        record: &T,
        edit: &ReviewEntry,
    ) -> anyhow::Result<()>
    where
        T: Clone + Into<ReviewRecord>,
    {
        let record: ReviewRecord = record.clone().into();
        if self
            .record_decision_state(record.scope_id(), record.kind(), record.id())?
            .pending
            .is_some_and(|pending| pending.revision == edit.revision)
        {
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
            actor: edit.actor.clone(),
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
            expected_revision: Some(edit.revision.clone()),
            revises: None,
        };
        let digest = request_digest(&input)?;
        let request_id = journal::new_id();
        let proposal_id = journal::new_id();
        let proposal_key = proposal_id.as_str().to_owned();
        crate::test_probes::at("requirement_submission_writing")?;
        guard::with_writer(
            &shards::proposal_cards_path(&self.layout, record.scope_id()),
            "*",
            || self.commit_submission(input, request_id, proposal_id, proposal_key, digest),
        )?;
        Ok(())
    }
}
