//! Guarded typed-spec changes to enrolled graph records.

use super::{classifier, decision_state::CycleFacts, new_id};
use crate::{
    cache::review_families,
    review::publication::with_record_state,
    shards,
    state_store::{RecordStamp, StateStore},
    write_error::SourceFailure,
};
use provenance_core::{review::ReviewRecord, ScopeId, StableId, Withdrawal};
use provenance_macros::rule;

struct TypedChange {
    before: ReviewRecord,
    after: ReviewRecord,
}

/// The records that a typed-spec apply deletes, and the undecided review
/// submissions of those records.
struct TypedDeletion {
    records: Vec<ReviewRecord>,
    undecided: Vec<StableId>,
}

impl StateStore {
    pub(crate) fn publish_typed_spec<R>(
        &self,
        scope: &ScopeId,
        actor: &str,
        publish: impl FnOnce(&Self) -> anyhow::Result<R>,
    ) -> anyhow::Result<R> {
        anyhow::ensure!(
            !actor.trim().is_empty(),
            "invalid typed-spec review identity"
        );
        with_record_state(self, RecordStamp::ResolveFromRepository, |staged| {
            // The staged copy holds the live records until `publish` runs.
            let current = review_families::review_records(staged, scope)?;
            let facts = CycleFacts::validated(staged, scope)?;
            let paths = review_families::review_paths(&staged.layout, scope);
            let paths = paths.iter().map(AsRef::as_ref).collect::<Vec<_>>();
            let result = staged.permitting(&paths, "*", publish)?;
            let desired = review_families::review_records(staged, scope)?;
            let (changes, deleted) = typed_changes(&current, &facts, &desired)?;
            staged.delete_record_discussions(scope, &deleted.records)?;
            staged.withdraw_deleted_submissions(scope, actor, &deleted.undecided)?;
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

    /// Withdraws each undecided review submission of a deleted record in the
    /// publication that deletes the record. The submissions stay as history.
    #[rule("rule_typed_deletion_withdraws_undecided_submissions")]
    fn withdraw_deleted_submissions(
        &self,
        scope: &ScopeId,
        actor: &str,
        undecided: &[StableId],
    ) -> anyhow::Result<()> {
        if undecided.is_empty() {
            return Ok(());
        }
        let path = shards::withdrawals_path(&self.layout, scope);
        self.permitting(&[&path], "*", |store| {
            for proposal in undecided {
                store.create_withdrawal(Withdrawal {
                    scope_id: scope.clone(),
                    id: new_id(),
                    proposal_id: proposal.clone(),
                    actor: actor.to_owned(),
                    reason: Some("Typed-spec apply deleted the record.".into()),
                })?;
            }
            Ok(())
        })
    }
}

fn typed_changes(
    current: &[ReviewRecord],
    facts: &CycleFacts,
    desired: &[ReviewRecord],
) -> anyhow::Result<(Vec<TypedChange>, TypedDeletion)> {
    let mut changes = Vec::new();
    let mut deleted = TypedDeletion {
        records: Vec::new(),
        undecided: Vec::new(),
    };
    for before in current {
        let after = desired
            .iter()
            .find(|record| record.kind() == before.kind() && record.id() == before.id());
        let Some(after) = after else {
            // A reviewer decision stays tied to its record, so a record with
            // an accepted or rejected submission is not deleted.
            if facts.has_decided_submission(before.kind(), before.id()) {
                return Err(enrolled_deletion_conflict(before));
            }
            deleted
                .undecided
                .extend(facts.undecided_submissions(before.kind(), before.id()));
            deleted.records.push(before.clone());
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

fn enrolled_deletion_conflict(before: &ReviewRecord) -> anyhow::Error {
    SourceFailure::wrap(
        crate::write_error::WriteFailure::EnrolledRecordDeletionConflict {
            record_kind: before.kind(),
            record_id: before.id().clone(),
        },
        anyhow::anyhow!(
            "cannot delete enrolled {} {} through typed-spec apply",
            before.kind().as_str(),
            before.id().as_str()
        ),
    )
}
