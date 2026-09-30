//! Guarded journal writes for typed-spec changes to enrolled graph records.

use super::{classifier, guard, journal, save::RecordEvidenceContext};
use crate::{
    cache::review_families, canonical_digest, publication::with_staged_state,
    state_store::StateStore, write_error::SourceFailure,
};
use provenance_core::{
    review::{ReviewEntry, ReviewRecord, REVIEW_SCHEMA_VERSION},
    NodeType, ScopeId, StableId,
};

struct TypedChange {
    before: Option<ReviewRecord>,
    after: ReviewRecord,
    head: Option<ReviewEntry>,
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
            if journal::record_digest(before)? == journal::record_digest(after)? {
                continue;
            }
            if before.schema_version() != REVIEW_SCHEMA_VERSION
                && !records_typed_occurrences(before.kind())
            {
                continue;
            }
            changes.push(TypedChange {
                before: Some(before.clone()),
                after: after.clone(),
                head: self.head(before)?,
            });
        }
        for after in desired {
            let exists = current
                .iter()
                .any(|record| record.kind() == after.kind() && record.id() == after.id());
            if exists || !records_typed_occurrences(after.kind()) {
                continue;
            }
            changes.push(TypedChange {
                before: None,
                after: after.clone(),
                head: self.typed_history_head(after)?,
            });
        }
        Ok(changes)
    }

    fn commit_typed_change(&self, actor: &str, change: &TypedChange) -> anyhow::Result<()> {
        let intent_digest = typed_intent(
            actor,
            change.before.as_ref(),
            &change.after,
            change.head.as_ref(),
        )?;
        let request_id = StableId::new(canonical_digest::sha256(
            format!("typed-spec-review\u{1f}{intent_digest}").as_bytes(),
        ))?;
        let entry = self.commit_record_evidence(
            change.before.as_ref(),
            &change.after,
            RecordEvidenceContext {
                head: change.head.clone(),
                actor: actor.to_owned(),
                request_id,
                intent_digest,
                origin: None,
            },
        )?;
        if classifier::changes_revision(entry.record_kind, &entry.changed_fields) {
            if let Some(requirement) = change.after.as_requirement() {
                self.commit_automatic_submission(requirement, &entry)?;
            }
        }
        Ok(())
    }

    fn typed_history_head(&self, record: &ReviewRecord) -> anyhow::Result<Option<ReviewEntry>> {
        let entries = self
            .review_entries(record.scope_id())?
            .into_iter()
            .filter(|entry| entry.record_kind == record.kind() && entry.record_id == *record.id())
            .collect::<Vec<_>>();
        journal::validated_head(&entries)
    }
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

fn typed_intent(
    actor: &str,
    before: Option<&ReviewRecord>,
    after: &ReviewRecord,
    head: Option<&ReviewEntry>,
) -> anyhow::Result<String> {
    let before = before.map(journal::record_digest).transpose()?;
    let after = journal::record_digest(after)?;
    let predecessor = head.map(|entry| entry.id.as_str());
    Ok(canonical_digest::digest(
        &canonical_digest::canonical_bytes(&(
            "typed-spec-review",
            actor,
            before,
            after,
            predecessor,
        ))?,
    ))
}
