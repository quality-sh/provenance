//! Automatic review submissions for graph record writes.

use super::{classifier, decision_state::check_request_size, SubmitRecordReview};
use crate::{shards, state_store::StateStore};
use provenance_core::review::ReviewRecord;
use provenance_macros::rule;

/// The longest summary an automatic submission copies from the record, so the
/// submission of a record at the read budget stays inside the budget.
const SUMMARY_CHARS: usize = 1000;

/// The submission actor of a server write that names no actor.
pub(super) const AUTHORING_ACTOR: &str = "authoring";

impl StateStore {
    /// Opens the review submission of a new record in the same write.
    pub(super) fn open_creation_submission(
        &self,
        created: &ReviewRecord,
        actor: &str,
    ) -> anyhow::Result<()> {
        let revision = classifier::review_revision(created.kind(), created)?;
        self.commit_automatic_submission(created, actor, &revision)
    }

    /// Opens the review submission of a content change in the same write.
    /// A lifecycle-only change keeps the review revision and opens no
    /// submission.
    pub(super) fn open_content_change_submission(
        &self,
        before: &ReviewRecord,
        after: &ReviewRecord,
        actor: &str,
    ) -> anyhow::Result<()> {
        let kind = after.kind();
        let revision = classifier::review_revision(kind, after)?;
        if classifier::review_revision(kind, before)? == revision {
            return Ok(());
        }
        self.commit_automatic_submission(after, actor, &revision)
    }

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
            .chars()
            .take(SUMMARY_CHARS)
            .collect::<String>();
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
        crate::test_probes::at("automatic_submission_before_commit")?;
        let path = shards::proposal_cards_path(&self.layout, record.scope_id());
        self.permitting(&[&path], "*", |store| store.commit_submission(input))?;
        Ok(())
    }
}
