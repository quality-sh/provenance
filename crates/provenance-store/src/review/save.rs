use super::{classifier, guard, journal, SaveRequirement};
use crate::{
    canonical_digest,
    publication::with_staged_state,
    shards,
    state_store::StateStore,
    write_error::{SourceFailure, WriteFailure},
};
use provenance_core::review::{
    RequirementEditState, ReviewEntry, ReviewRecord, SaveOutcome, REVIEW_SCHEMA_VERSION,
};
use provenance_core::{Requirement, ScopeId, StableId};
use provenance_macros::rule;

pub(super) struct RecordEvidenceContext {
    pub(super) head: Option<ReviewEntry>,
    pub(super) actor: String,
    pub(super) request_id: StableId,
    pub(super) intent_digest: String,
    pub(super) origin: Option<provenance_core::threads::DiscussionOrigin>,
}

impl StateStore {
    pub fn requirement_edit_state(
        &self,
        scope: &ScopeId,
        id: &StableId,
    ) -> anyhow::Result<RequirementEditState> {
        self.record_edit_state(scope, provenance_core::NodeType::Requirement, id)
    }

    pub fn record_edit_state(
        &self,
        scope: &ScopeId,
        kind: provenance_core::NodeType,
        id: &StableId,
    ) -> anyhow::Result<RequirementEditState> {
        self.with_repository_publication(|| {
            let record = crate::cache::review_families::record(self, scope, kind, id)?;
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
    pub fn save_requirement(&self, input: SaveRequirement) -> anyhow::Result<ReviewEntry> {
        self.save_requirement_with_origin(input, None, |_, entry| Ok(entry))
    }

    pub(crate) fn save_requirement_resource(
        &self,
        input: SaveRequirement,
    ) -> anyhow::Result<super::RequirementResourceSnapshot> {
        self.save_requirement_with_origin(input, None, |store, entry| {
            store.requirement_resource_snapshot_unlocked(&entry.scope_id, &entry.record_id)
        })
    }

    /// Saves a Requirement edit as a Discussion outcome and keeps the previous
    /// text with it, so the outcome shows the change from the record's earlier
    /// state through the Before snapshot rather than a link to the latest text.
    #[rule("rule_discussion_outcome_shows_record_change")]
    pub fn save_requirement_from_discussion(
        &self,
        input: SaveRequirement,
        origin: provenance_core::threads::DiscussionOrigin,
    ) -> anyhow::Result<ReviewEntry> {
        self.save_requirement_with_origin(input, Some(origin), |_, entry| Ok(entry))
    }

    fn save_requirement_with_origin<R>(
        &self,
        mut input: SaveRequirement,
        origin: Option<provenance_core::threads::DiscussionOrigin>,
        complete: impl FnOnce(&Self, ReviewEntry) -> anyhow::Result<R>,
    ) -> anyhow::Result<R> {
        anyhow::ensure!(
            !input.actor.trim().is_empty(),
            "invalid review request identity"
        );
        anyhow::ensure!(
            serde_json::to_vec(&input)?.len() <= 1_048_576,
            "review request exceeds the operation byte budget"
        );
        input.normalize();
        let intent_digest = canonical_digest::digest(&match &origin {
            Some(origin) => canonical_digest::canonical_bytes(&(&input, origin))?,
            None => canonical_digest::canonical_bytes(&input)?,
        });
        self.with_repository_publication(|| {
            crate::test_probes::at("requirement_save_locked")?;
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
                        && receipt.record_id == input.update.id
                        && receipt.actor == input.actor,
                    "review request ID was reused with different intent"
                );
                return complete(self, receipt);
            }
            if let Some(origin) = &origin {
                self.validate_discussion_origin(scope, origin)?;
            }
            self.validated_review_entries(scope)?;
            let review_record = provenance_core::review::ReviewRecord::from(record.clone());
            let head = self.head(&review_record)?;
            let current_etag = head
                .as_ref()
                .map(|e| e.etag.clone())
                .unwrap_or(journal::etag(&review_record, None)?);
            if input.expected_etag != current_etag {
                return Err(SourceFailure::wrap(
                    WriteFailure::RequirementEditConflict { current_etag },
                    anyhow::anyhow!("stale Requirement edit etag"),
                ));
            }
            let stamp = self.current_record_stamp()?;
            with_staged_state(&self.layout, false, |layout| {
                let staged = Self::staged(layout.clone(), stamp);
                let path = shards::requirements_path(layout, scope);
                let record_id = record.id.clone();
                guard::with_writer(&path, record_id.as_str(), || {
                    let entry =
                        staged.commit_requirement(input, &record, head, intent_digest, origin)?;
                    complete(&staged, entry)
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
        origin: Option<provenance_core::threads::DiscussionOrigin>,
    ) -> anyhow::Result<ReviewEntry> {
        let scope = before.scope_id.clone();
        let id = before.id.clone();
        let actor = input.actor;
        let request_id = input.request_id;
        self.apply_requirement_update(input.update)?;
        if let Some(relationships) = input.relationships {
            crate::test_probes::at("requirement_relationships_expanding")?;
            self.replace_review_relationships(&scope, &id, &relationships)?;
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
        let before_record = ReviewRecord::from(before.clone());
        let after_record = ReviewRecord::from(after.clone());
        let entry = self.commit_record_evidence(
            Some(&before_record),
            &after_record,
            RecordEvidenceContext {
                head,
                actor,
                request_id,
                intent_digest,
                origin,
            },
        )?;
        if classifier::changes_revision(entry.record_kind, &entry.changed_fields) {
            self.commit_automatic_submission(&after, &entry)?;
        }
        Ok(entry)
    }

    pub(super) fn commit_record_evidence(
        &self,
        before: Option<&ReviewRecord>,
        after: &ReviewRecord,
        context: RecordEvidenceContext,
    ) -> anyhow::Result<ReviewEntry> {
        let RecordEvidenceContext {
            head,
            actor,
            request_id,
            intent_digest,
            origin,
        } = context;
        let kind = after.kind();
        if let Some(before) = before {
            anyhow::ensure!(
                before.kind() == kind
                    && after.scope_id() == before.scope_id()
                    && after.id() == before.id(),
                "a review save cannot change its record address"
            );
        }
        let scope = after.scope_id().clone();
        let id = after.id().clone();
        let fields = match before {
            Some(before) => classifier::changed_fields(kind, before, after)?,
            None => serde_json::to_value(after)?
                .as_object()
                .unwrap()
                .keys()
                .filter(|key| key.as_str() != "schema_version")
                .cloned()
                .collect(),
        };
        let outcome = if before.is_none() {
            SaveOutcome::Created
        } else if head.is_none() {
            SaveOutcome::Enrolled
        } else if fields.is_empty() {
            SaveOutcome::NoChange
        } else if classifier::changes_revision(kind, &fields) {
            SaveOutcome::Changed
        } else {
            SaveOutcome::LifecycleOnly
        };
        let revision = match &head {
            Some(head) if !classifier::changes_revision(kind, &fields) => head.revision.clone(),
            _ => journal::new_id(),
        };
        let before_snapshot = match (before, &head) {
            (None, _) => None,
            (_, Some(entry)) => Some(entry.after.clone()),
            (Some(before), None) => Some(journal::snapshot(&self.layout, before)?),
        };
        let after_snapshot = if outcome == SaveOutcome::NoChange {
            before_snapshot
                .clone()
                .expect("a no-change occurrence has a prior snapshot")
        } else {
            journal::snapshot(&self.layout, after)?
        };
        let entry_id = journal::new_id();
        let etag = if outcome == SaveOutcome::NoChange {
            head.as_ref().unwrap().etag.clone()
        } else {
            journal::etag(after, Some(&entry_id))?
        };
        let entry = ReviewEntry {
            schema_version: REVIEW_SCHEMA_VERSION,
            scope_id: scope.clone(),
            record_kind: kind,
            record_id: id,
            sequence: head.as_ref().map_or(1, |entry| entry.sequence + 1),
            id: entry_id,
            predecessor: head.as_ref().map(|e| e.id.clone()),
            revision,
            prior_revision: head.map(|e| e.revision),
            before: before_snapshot,
            after: after_snapshot,
            changed_fields: fields,
            actor,
            request_id,
            intent_digest,
            etag,
            outcome,
            origin,
        };
        anyhow::ensure!(
            serde_json::to_vec(&entry)?.len() as u64 <= journal::ENTRY_BYTES,
            "review receipt exceeds the entry byte budget"
        );
        journal::write_new(
            &journal::entry_path(&self.layout, &scope, &entry.request_id),
            &entry,
        )?;
        if classifier::changes_revision(kind, &entry.changed_fields) {
            self.commit_automatic_submission(after, &entry)?;
        }
        Ok(entry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::ProvenanceLayout;
    use camino::Utf8Path;
    use provenance_core::review::SaveOutcome;
    use serde_json::json;

    fn fixture() -> (tempfile::TempDir, StateStore, ScopeId) {
        let temp = tempfile::tempdir().unwrap();
        let layout = ProvenanceLayout::new(Utf8Path::from_path(temp.path()).unwrap());
        std::fs::create_dir_all(layout.state_dir()).unwrap();
        std::fs::write(
            layout.manifest_path(),
            r#"{"schema_version":2,"scopes":[{"id":"default","path_prefix":"."}]}"#,
        )
        .unwrap();
        (
            temp,
            StateStore::new(layout),
            ScopeId::new("default").unwrap(),
        )
    }

    fn enroll(store: &StateStore, id: &StableId, request: &str, update: &serde_json::Value) {
        let input: SaveRequirement = serde_json::from_value(json!({
            "request_id":request, "actor":"ben",
            "expected_etag":store.requirement_edit_state(&ScopeId::new("default").unwrap(), id).unwrap().etag,
            "update":update, "relationships":null
        }))
        .unwrap();
        assert_eq!(
            store.save_requirement(input).unwrap().outcome,
            SaveOutcome::Enrolled
        );
    }

    #[test]
    fn first_enrollment_submits_only_when_review_content_changes() {
        let (_temp, store, scope) = fixture();
        for id in ["req_changed", "req_unchanged"] {
            store
                .write_requirement(
                    serde_json::from_value(json!({
                        "scope_id":"default", "id":id,
                        "statement":format!("The system stores {id}."),
                        "status":"discovery", "depends_on":[], "supersedes":[]
                    }))
                    .unwrap(),
                )
                .unwrap();
        }
        let changed = StableId::new("req_changed").unwrap();
        enroll(
            &store,
            &changed,
            "enroll_changed",
            &json!({"scope_id":"default", "id":"req_changed", "description":"Needs audit."}),
        );
        assert!(store
            .requirement_decision_state(&scope, &changed)
            .unwrap()
            .pending
            .is_some());

        let unchanged = StableId::new("req_unchanged").unwrap();
        enroll(
            &store,
            &unchanged,
            "enroll_unchanged",
            &json!({"scope_id":"default", "id":"req_unchanged"}),
        );
        assert!(store
            .requirement_decision_state(&scope, &unchanged)
            .unwrap()
            .pending
            .is_none());
    }

    #[test]
    fn record_save_evidence_uses_the_record_kind() {
        let (_temp, store, _) = fixture();
        let before = serde_json::from_value::<provenance_core::Source>(json!({
            "schema_version": 3,
            "scope_id": "default",
            "id": "source_a",
            "name": "Policy A",
            "source_type": "document",
            "url": null
        }))
        .unwrap();
        let mut after = before.clone();
        after.name = "Policy B".into();
        let path = crate::shards::sources_path(&store.layout, &after.scope_id);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut bytes = serde_json::to_vec(&after).unwrap();
        bytes.push(b'\n');
        std::fs::write(path, bytes).unwrap();

        let before = ReviewRecord::from(before);
        let after = ReviewRecord::from(after);
        let entry = store
            .commit_record_evidence(
                Some(&before),
                &after,
                RecordEvidenceContext {
                    head: None,
                    actor: "reviewer".into(),
                    request_id: StableId::new("save-source-a").unwrap(),
                    intent_digest: "sha256:intent".into(),
                    origin: None,
                },
            )
            .unwrap();

        assert_eq!(entry.record_kind, provenance_core::NodeType::Source);
        assert_eq!(entry.record_id.as_str(), "source_a");
        assert_eq!(entry.changed_fields, ["name"]);
    }
}
