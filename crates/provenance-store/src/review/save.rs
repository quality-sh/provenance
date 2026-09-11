use super::{classifier, guard, journal, SaveRequirement};
use crate::{canonical_digest, publication::with_staged_state, shards, state_store::StateStore};
use provenance_core::review::{
    RequirementEditState, ReviewEntry, SaveOutcome, REVIEW_SCHEMA_VERSION,
};
use provenance_core::{Requirement, ScopeId, StableId};

impl StateStore {
    pub fn requirement_edit_state(
        &self,
        scope: &ScopeId,
        id: &StableId,
    ) -> anyhow::Result<RequirementEditState> {
        self.with_repository_publication(|| {
            let record = self.requirement(scope, id)?;
            let head = self.head(&record)?;
            Ok(RequirementEditState {
                etag: head
                    .as_ref()
                    .map(|e| e.etag.clone())
                    .unwrap_or(journal::etag(&record, None)?),
                revision: head.as_ref().map(|e| e.revision.clone()),
                snapshot: head.map(|e| e.after),
            })
        })
    }

    /// Publishes a Requirement edit, its evidence, and its request receipt together.
    pub fn save_requirement(&self, mut input: SaveRequirement) -> anyhow::Result<ReviewEntry> {
        anyhow::ensure!(
            !input.actor.trim().is_empty(),
            "invalid review request identity"
        );
        anyhow::ensure!(
            serde_json::to_vec(&input)?.len() <= 1_048_576,
            "review request exceeds the operation byte budget"
        );
        input.normalize();
        let intent_digest = canonical_digest::digest(&canonical_digest::canonical_bytes(&input)?);
        self.with_repository_publication(|| {
            let scope_id = input.update.scope_id.clone();
            let scope = &scope_id;
            anyhow::ensure!(
                self.manifest()?.scopes.iter().any(|s| s.id == *scope),
                "review scope is not in the manifest"
            );
            let record = self.requirement(scope, &input.update.id)?;
            super::owner_matches(&record, input.update.declared_by.as_deref())?;
            let receipt_path = journal::entry_path(&self.layout, scope, &input.request_id);
            if receipt_path.try_exists()? {
                let receipt = journal::read_entry(&self.layout, &receipt_path)?;
                anyhow::ensure!(
                    receipt.scope_id == *scope
                        && receipt.request_id == input.request_id
                        && receipt.intent_digest == intent_digest
                        && receipt.requirement_id == input.update.id
                        && receipt.actor == input.actor,
                    "review request ID was reused with different intent"
                );
                return Ok(receipt);
            }
            self.validated_review_entries(scope)?;
            let head = self.head(&record)?;
            let current_etag = head
                .as_ref()
                .map(|e| e.etag.clone())
                .unwrap_or(journal::etag(&record, None)?);
            anyhow::ensure!(
                input.expected_etag == current_etag,
                "stale Requirement edit etag"
            );
            with_staged_state(&self.layout, false, |layout| {
                let staged = Self::new(layout.clone());
                let path = shards::requirements_path(layout, scope);
                let record_id = record.id.clone();
                guard::with_writer(&path, record_id.as_str(), || {
                    staged.commit_requirement(input, &record, head, intent_digest)
                })
            })
        })
    }

    fn commit_requirement(
        &self,
        input: SaveRequirement,
        before: &Requirement,
        head: Option<ReviewEntry>,
        intent_digest: String,
    ) -> anyhow::Result<ReviewEntry> {
        let scope = before.scope_id.clone();
        let id = before.id.clone();
        self.update_requirement(input.update)?;
        if let Some(relationships) = input.relationships {
            self.replace_review_relationships(&scope, &id, relationships)?;
        }
        let path = shards::requirements_path(&self.layout, &scope);
        let after = self.mutate_jsonl_records(&path, |records: &mut Vec<Requirement>| {
            let record = records.iter_mut().find(|r| r.id == id).unwrap();
            record.schema_version = REVIEW_SCHEMA_VERSION;
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
        let outcome = if head.is_none() {
            SaveOutcome::Enrolled
        } else if fields.is_empty() {
            SaveOutcome::NoChange
        } else if classifier::changes_revision(&fields) {
            SaveOutcome::Changed
        } else {
            SaveOutcome::LifecycleOnly
        };
        let revision = match &head {
            Some(head) if !classifier::changes_revision(&fields) => head.revision.clone(),
            _ => journal::new_id(),
        };
        let before_snapshot = match &head {
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
            head.as_ref().unwrap().etag.clone()
        } else {
            journal::etag(&after, Some(&entry_id))?
        };
        let entry = ReviewEntry {
            schema_version: REVIEW_SCHEMA_VERSION,
            scope_id: scope.clone(),
            requirement_id: id,
            sequence: head.as_ref().map_or(1, |e| e.sequence + 1),
            id: entry_id,
            predecessor: head.as_ref().map(|e| e.id.clone()),
            revision,
            prior_revision: head.map(|e| e.revision),
            before: Some(before_snapshot),
            after: after_snapshot,
            changed_fields: fields,
            actor: input.actor,
            request_id: input.request_id,
            intent_digest,
            etag,
            outcome,
        };
        anyhow::ensure!(
            serde_json::to_vec(&entry)?.len() as u64 <= journal::ENTRY_BYTES,
            "review receipt exceeds the entry byte budget"
        );
        journal::write_new(
            &journal::entry_path(&self.layout, &scope, &entry.request_id),
            &entry,
        )?;
        Ok(entry)
    }
}
