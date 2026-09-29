//! Guarded journal writes for typed-spec changes to enrolled graph records.

use super::{classifier, guard, journal, save::RecordEvidenceContext};
use crate::{
    cache::review_families, canonical_digest, publication::with_staged_state,
    state_store::StateStore, write_error::SourceFailure,
};
use provenance_core::{
    review::{ReviewEntry, ReviewRecord, REVIEW_SCHEMA_VERSION},
    ScopeId, StableId,
};

struct TypedChange {
    before: ReviewRecord,
    after: ReviewRecord,
    head: ReviewEntry,
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
        for before in current
            .into_iter()
            .filter(|record| record.schema_version() == REVIEW_SCHEMA_VERSION)
        {
            let after = desired
                .iter()
                .find(|record| record.kind() == before.kind() && record.id() == before.id())
                .ok_or_else(|| {
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
                })?;
            if journal::record_digest(&before)? == journal::record_digest(after)? {
                continue;
            }
            let head = self
                .head(&before)?
                .expect("enrolled records have validated review history");
            changes.push(TypedChange {
                before,
                after: after.clone(),
                head,
            });
        }
        Ok(changes)
    }

    fn commit_typed_change(&self, actor: &str, change: &TypedChange) -> anyhow::Result<()> {
        let intent_digest = typed_intent(actor, &change.before, &change.after)?;
        let request_id = StableId::new(canonical_digest::sha256(
            format!("typed-spec-review\u{1f}{intent_digest}").as_bytes(),
        ))?;
        let entry = self.commit_record_evidence(
            &change.before,
            &change.after,
            RecordEvidenceContext {
                head: Some(change.head.clone()),
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
}

fn review_records(
    layout: &crate::layout::ProvenanceLayout,
    scope: &ScopeId,
) -> anyhow::Result<Vec<ReviewRecord>> {
    review_families::review_records(&StateStore::new(layout.clone()), scope)
}

fn typed_intent(
    actor: &str,
    before: &ReviewRecord,
    after: &ReviewRecord,
) -> anyhow::Result<String> {
    let before = journal::record_digest(before)?;
    let after = journal::record_digest(after)?;
    Ok(canonical_digest::digest(
        &canonical_digest::canonical_bytes(&("typed-spec-review", actor, before, after))?,
    ))
}
