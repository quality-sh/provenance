//! Guarded journal writes for typed-spec adoption of enrolled Requirements.

use super::{classifier, guard, journal};
use crate::{canonical_digest, publication::with_staged_state, shards, state_store::StateStore};
use provenance_core::review::{ReviewEntry, SaveOutcome, REVIEW_SCHEMA_VERSION};
use provenance_core::{Requirement, ScopeId, StableId};

impl StateStore {
    pub(crate) fn commit_enrollment_adoptions(
        &self,
        scope: &ScopeId,
        desired: &[Requirement],
        owner: &str,
    ) -> anyhow::Result<()> {
        let current = self.list_requirements(scope)?;
        for record in desired {
            let Some(before) = current.iter().find(|r| r.id == record.id) else {
                continue;
            };
            // An owned record stays with the guarded shard-swap refusal.
            if before.declared_by.is_some()
                || before.schema_version != REVIEW_SCHEMA_VERSION
                || before == record
            {
                continue;
            }
            let intent = adoption_intent(owner, before, record)?;
            let request_id = StableId::new(canonical_digest::sha256(
                format!("typed-spec-adoption\u{1f}{intent}").as_bytes(),
            ))?;
            self.commit_adoption(before, record, owner, request_id, &intent)?;
        }
        Ok(())
    }

    fn commit_adoption(
        &self,
        before: &Requirement,
        after: &Requirement,
        actor: &str,
        request_id: StableId,
        intent_digest: &str,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            !actor.trim().is_empty(),
            "invalid adoption request identity"
        );
        self.with_repository_publication(|| {
            let scope = before.scope_id.clone();
            let path = journal::entry_path(&self.layout, &scope, &request_id);
            if path.try_exists()? {
                let receipt = journal::read_entry(&self.layout, &path)?;
                anyhow::ensure!(
                    receipt.scope_id == scope
                        && receipt.requirement_id == before.id
                        && receipt.request_id == request_id
                        && receipt.intent_digest == intent_digest,
                    "adoption request ID was reused with different intent"
                );
                return Ok(());
            }
            self.validated_review_entries(&scope)?;
            let head = self.head(before)?;
            with_staged_state(&self.layout, false, |layout| {
                let staged = Self::new(layout.clone());
                let path = shards::requirements_path(layout, &scope);
                let id = before.id.clone();
                guard::with_writer(&path, id.as_str(), || {
                    staged.commit_adoption_record(
                        before,
                        after,
                        actor,
                        request_id,
                        intent_digest,
                        head.as_ref(),
                    )
                })
            })
        })
    }

    fn commit_adoption_record(
        &self,
        before: &Requirement,
        after: &Requirement,
        actor: &str,
        request_id: StableId,
        intent_digest: &str,
        head: Option<&ReviewEntry>,
    ) -> anyhow::Result<()> {
        let scope = before.scope_id.clone();
        let id = before.id.clone();
        let path = shards::requirements_path(&self.layout, &scope);
        let after = self.mutate_jsonl_records(&path, |records: &mut Vec<Requirement>| {
            let record = records
                .iter_mut()
                .find(|r| r.id == id)
                .ok_or_else(|| anyhow::anyhow!("adopted Requirement left the scope"))?;
            *record = after.clone();
            record.schema_version = REVIEW_SCHEMA_VERSION;
            crate::state_store::read_budget::ensure_within_read_budget(record)?;
            Ok(record.clone())
        })?;
        self.validate_graph_scope(&scope)?;
        let mut manifest = self.manifest()?;
        manifest.schema_version = REVIEW_SCHEMA_VERSION;
        std::fs::write(
            self.layout.manifest_path(),
            serde_json::to_vec_pretty(&manifest)?,
        )?;
        let fields = classifier::changed_fields(before, &after)?;
        let outcome = if fields.is_empty() {
            SaveOutcome::NoChange
        } else if classifier::changes_revision(&fields) {
            SaveOutcome::Changed
        } else {
            SaveOutcome::LifecycleOnly
        };
        let revision = match head {
            Some(head) if !classifier::changes_revision(&fields) => head.revision.clone(),
            _ => journal::new_id(),
        };
        let before_snapshot = match head {
            Some(entry) => entry.after.clone(),
            None => journal::snapshot(&self.layout, before)?,
        };
        let after_snapshot = if outcome == SaveOutcome::NoChange {
            before_snapshot.clone()
        } else {
            journal::snapshot(&self.layout, &after)?
        };
        let entry_id = journal::new_id();
        let etag = if outcome == SaveOutcome::NoChange {
            match head {
                Some(head) => head.etag.clone(),
                None => journal::etag(&after, None)?,
            }
        } else {
            journal::etag(&after, Some(&entry_id))?
        };
        let entry = ReviewEntry {
            schema_version: REVIEW_SCHEMA_VERSION,
            scope_id: scope.clone(),
            requirement_id: id,
            sequence: head.map_or(1, |e| e.sequence + 1),
            id: entry_id,
            predecessor: head.map(|e| e.id.clone()),
            revision,
            prior_revision: head.map(|e| e.revision.clone()),
            before: Some(before_snapshot),
            after: after_snapshot,
            changed_fields: fields,
            actor: actor.to_owned(),
            request_id,
            intent_digest: intent_digest.to_owned(),
            etag,
            outcome,
            origin: None,
        };
        anyhow::ensure!(
            serde_json::to_vec(&entry)?.len() as u64 <= journal::ENTRY_BYTES,
            "review receipt exceeds the entry byte budget"
        );
        journal::write_new(
            &journal::entry_path(&self.layout, &scope, &entry.request_id),
            &entry,
        )?;
        Ok(())
    }
}

/// Excludes record stamps so retries resolve to the same receipt.
fn adoption_intent(
    owner: &str,
    before: &Requirement,
    after: &Requirement,
) -> anyhow::Result<String> {
    let before = journal::record_digest(before)?;
    let after = journal::record_digest(after)?;
    Ok(canonical_digest::digest(
        &canonical_digest::canonical_bytes(&("typed-spec-adoption", owner, before, after))?,
    ))
}
