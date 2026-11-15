//! Guarded typed-spec changes to enrolled graph records.

use super::{classifier, decision_state::CycleFacts};
use crate::{
    families::review_families,
    review::publication::with_record_state,
    state_store::{RecordStamp, ReviewHistory, StateStore},
};
use provenance_core::{review::ReviewRecord, ScopeId};

struct TypedChange {
    before: ReviewRecord,
    after: ReviewRecord,
}

impl StateStore {
    /// Reads the review history that a typed-spec decision checks before it
    /// deletes a record, and refuses a Withdrawal that names no review
    /// submission.
    pub(crate) fn review_history(&self, scope: &ScopeId) -> anyhow::Result<ReviewHistory> {
        let facts = CycleFacts::validated(self, scope)?;
        Ok(ReviewHistory {
            submitted: facts
                .submitted_records()
                .map(|(kind, id)| (kind, id.clone()))
                .collect(),
            dispositions: facts.dispositions().to_vec(),
        })
    }

    pub(crate) fn publish_typed_spec<R>(
        &self,
        scope: &ScopeId,
        actor: &str,
        publish: impl FnOnce(&Self) -> anyhow::Result<R>,
    ) -> anyhow::Result<R> {
        if actor.trim().is_empty() {
            return Err(crate::refusal::refuse!(
                (crate::write_error::WriteFailure::invalid_field("actor")),
                "invalid typed-spec review identity"
            ));
        }
        with_record_state(self, RecordStamp::ResolveFromRepository, |staged| {
            // The staged copy holds the live records until `publish` runs.
            let current = review_families::review_records(staged, scope)?;
            let paths = review_families::review_paths(&staged.layout, scope);
            let paths = paths.iter().map(AsRef::as_ref).collect::<Vec<_>>();
            let result = staged.permitting(&paths, "*", publish)?;
            let desired = review_families::review_records(staged, scope)?;
            let (changes, deleted) = typed_changes(&current, &desired)?;
            staged.delete_record_discussions(scope, &deleted)?;
            for change in &changes {
                staged.commit_typed_change(actor, change)?;
            }
            Ok(result)
        })
    }

    /// Opens the review submission of a typed change to record content.
    fn commit_typed_change(&self, actor: &str, change: &TypedChange) -> anyhow::Result<()> {
        // Refuses a changed field that has no review classification.
        classifier::changed_fields(change.after.kind(), &change.before, &change.after)?;
        self.open_content_change_submission(&change.before, &change.after, actor)
    }
}

/// The changed records and the deleted records of one publication. The
/// typed-spec decision has refused each deletion that review history keeps.
fn typed_changes(
    current: &[ReviewRecord],
    desired: &[ReviewRecord],
) -> anyhow::Result<(Vec<TypedChange>, Vec<ReviewRecord>)> {
    let mut changes = Vec::new();
    let mut deleted = Vec::new();
    for before in current {
        let after = desired
            .iter()
            .find(|record| record.kind() == before.kind() && record.id() == before.id());
        let Some(after) = after else {
            deleted.push(before.clone());
            continue;
        };
        if content(before)? == content(after)? {
            continue;
        }
        changes.push(TypedChange {
            before: before.clone(),
            after: after.clone(),
        });
    }
    Ok((changes, deleted))
}

/// The record without its record stamps.
fn content(record: &ReviewRecord) -> serde_json::Result<serde_json::Value> {
    provenance_core::model::record_stamps::content_value(record)
}
