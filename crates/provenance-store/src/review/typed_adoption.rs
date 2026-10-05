//! Guarded typed-spec changes to enrolled graph records.

use super::{classifier, guard};
use crate::{
    cache::review_families, publication::with_staged_state, state_store::StateStore,
    write_error::SourceFailure,
};
use provenance_core::{
    review::{ReviewRecord, REVIEW_SCHEMA_VERSION},
    NodeType, ScopeId,
};

struct TypedChange {
    before: ReviewRecord,
    after: ReviewRecord,
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
        with_staged_state(&self.layout, false, |layout| {
            let staged = Self::new(layout.clone());
            let paths = review_families::review_paths(layout, scope);
            let result = guard::with_writers(&paths, "*", || publish(&staged))?;
            let desired = review_families::review_records(&staged, scope)?;
            let changes = self.typed_changes(scope, &desired)?;
            for change in &changes {
                staged.commit_typed_change(actor, change)?;
            }
            Ok(result)
        })
    }

    fn typed_changes(
        &self,
        scope: &ScopeId,
        desired: &[ReviewRecord],
    ) -> anyhow::Result<Vec<TypedChange>> {
        let current = review_records(&self.layout, scope)?;
        let mut changes = Vec::new();
        for before in &current {
            let after = desired
                .iter()
                .find(|record| record.kind() == before.kind() && record.id() == before.id());
            let Some(after) = after else {
                if before.schema_version() == REVIEW_SCHEMA_VERSION {
                    return Err(enrolled_deletion_conflict(before));
                }
                continue;
            };
            if content(before)? == content(after)? {
                continue;
            }
            if before.schema_version() != REVIEW_SCHEMA_VERSION
                && !records_typed_occurrences(before.kind())
            {
                continue;
            }
            changes.push(TypedChange {
                before: before.clone(),
                after: after.clone(),
            });
        }
        Ok(changes)
    }

    /// Opens the review submission of a typed change to Requirement content.
    fn commit_typed_change(&self, actor: &str, change: &TypedChange) -> anyhow::Result<()> {
        let kind = change.after.kind();
        let fields = classifier::changed_fields(kind, &change.before, &change.after)?;
        if classifier::changes_revision(kind, &fields) {
            if let Some(requirement) = change.after.as_requirement() {
                let revision = classifier::review_revision(kind, &change.after)?;
                self.commit_automatic_submission(requirement, actor, &revision)?;
            }
        }
        Ok(())
    }
}

/// The record without its record stamps.
fn content(record: &ReviewRecord) -> serde_json::Result<serde_json::Value> {
    provenance_core::model::record_stamps::content_value(record)
}

const fn records_typed_occurrences(kind: NodeType) -> bool {
    matches!(kind, NodeType::Source | NodeType::Rule)
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

fn review_records(
    layout: &crate::layout::ProvenanceLayout,
    scope: &ScopeId,
) -> anyhow::Result<Vec<ReviewRecord>> {
    review_families::review_records(&StateStore::new(layout.clone()), scope)
}
