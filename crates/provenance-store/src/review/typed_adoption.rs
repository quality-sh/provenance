//! Guarded journal writes for typed-spec changes to enrolled graph records.

use super::{classifier, guard, journal, save::RecordEvidenceContext};
use crate::{
    cache::review_families,
    canonical_digest,
    publication::with_staged_state,
    shards,
    state_store::StateStore,
};
use provenance_core::{
    review::{ReviewEntry, ReviewRecord, REVIEW_SCHEMA_VERSION},
    NodeType, ScopeId, StableId,
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
        desired: Vec<ReviewRecord>,
        publish: impl FnOnce(&Self) -> anyhow::Result<R>,
    ) -> anyhow::Result<R> {
        anyhow::ensure!(
            !actor.trim().is_empty(),
            "invalid typed-spec review identity"
        );
        let changes = self.typed_changes(scope, &desired)?;
        if changes.is_empty() {
            return publish(self);
        }
        with_staged_state(&self.layout, false, |layout| {
            let staged = Self::new(layout.clone());
            for change in &changes {
                staged.commit_typed_change(actor, change)?;
            }
            publish(&staged)
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
                    anyhow::anyhow!(
                        "cannot delete enrolled {} {} through typed-spec apply",
                        before.kind().as_str(),
                        before.id().as_str()
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
        let scope = change.before.scope_id();
        let kind = change.before.kind();
        let id = change.before.id();
        let path = shards::path_for(&self.layout, scope, kind);
        guard::with_writer(&path, id.as_str(), || {
            self.mutate_jsonl_records(&path, |records: &mut Vec<serde_json::Value>| {
                let record = records
                    .iter_mut()
                    .find(|record| record["id"] == id.as_str())
                    .ok_or_else(|| anyhow::anyhow!("typed-spec record left the scope"))?;
                *record = serde_json::to_value(&change.after)?;
                Ok(())
            })
        })?;
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
    let mut records = Vec::new();
    for kind in NodeType::ALL {
        let path = shards::path_for(layout, scope, kind);
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        };
        for line in text.lines() {
            let value = serde_json::from_str(line)?;
            records.push(review_families::deserialize_record(kind, &value)?);
        }
    }
    Ok(records)
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
