//! Automatic review submissions for public Requirement writes.

use super::{decision_state::request_digest, guard, journal, SubmitRequirementReview};
use crate::{shards, state_store::StateStore};
use provenance_core::{review::ReviewEntry, Requirement};

impl StateStore {
    /// Binds a server-created review submission to a new content revision.
    pub(super) fn commit_automatic_submission(
        &self,
        record: &Requirement,
        edit: &ReviewEntry,
    ) -> anyhow::Result<()> {
        let input = SubmitRequirementReview {
            scope_id: record.scope_id.clone(),
            actor: edit.actor.clone(),
            requirement_id: record.id.clone(),
            declared_by: record.declared_by.clone(),
            title: format!("Review {}", record.id.as_str()),
            summary: record.statement.clone(),
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
            &shards::proposal_cards_path(&self.layout, &record.scope_id),
            "*",
            || self.commit_submission(input, request_id, proposal_id, proposal_key, digest),
        )?;
        Ok(())
    }
}
