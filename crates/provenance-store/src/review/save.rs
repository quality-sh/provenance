use super::{classifier, guard, SaveRequirement};
use crate::{
    review::publication::with_record_state,
    shards,
    state_store::StateStore,
    write_error::{SourceFailure, WriteFailure},
};
use provenance_core::review::{RequirementEditState, ReviewRecord};
use provenance_core::threads::DiscussionOrigin;
use provenance_core::{NodeType, Requirement, ScopeId, StableId};
use provenance_macros::rule;

/// The edit precondition is a digest of the record without its stored format
/// marker or record stamps.
pub(super) fn etag(record: &ReviewRecord) -> anyhow::Result<String> {
    let mut content = provenance_core::model::record_stamps::content_value(record)?;
    content
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("record does not serialize to an object"))?
        .remove("schema_version");
    Ok(crate::canonical_digest::digest(
        &crate::canonical_digest::canonical_bytes(&content)?,
    ))
}

/// The review revision depends only on the review content of the record.
pub(super) fn current_revision(record: &ReviewRecord) -> anyhow::Result<Option<StableId>> {
    classifier::review_revision(record.kind(), record).map(Some)
}

impl StateStore {
    pub(super) fn requirement(
        &self,
        scope: &ScopeId,
        id: &StableId,
    ) -> anyhow::Result<Requirement> {
        let records = self.list_requirements(scope)?;
        let matches = records.iter().filter(|r| r.id == *id).collect::<Vec<_>>();
        anyhow::ensure!(
            matches.len() == 1 && matches[0].scope_id == *scope,
            "requirement {} does not exist uniquely in this scope",
            id.as_str()
        );
        Ok(matches[0].clone())
    }

    pub fn requirement_edit_state(
        &self,
        scope: &ScopeId,
        id: &StableId,
    ) -> anyhow::Result<RequirementEditState> {
        self.with_repository_publication(|| {
            let record = self.requirement(scope, id)?;
            Self::record_edit_state_for_record(&record.into())
        })
    }

    pub fn record_edit_state(
        &self,
        scope: &ScopeId,
        kind: NodeType,
        id: &StableId,
    ) -> anyhow::Result<RequirementEditState> {
        self.with_repository_publication(|| {
            let record = crate::cache::review_families::record(self, scope, kind, id)?;
            Self::record_edit_state_for_record(&record)
        })
    }

    pub(super) fn record_edit_state_for_record(
        record: &ReviewRecord,
    ) -> anyhow::Result<RequirementEditState> {
        Ok(RequirementEditState {
            etag: etag(record)?,
            revision: current_revision(record)?,
        })
    }

    /// Publishes a Requirement edit and the review state it opens together.
    pub fn save_requirement(&self, input: SaveRequirement) -> anyhow::Result<RequirementEditState> {
        self.save_requirement_with_origin(input, None, |_, after| {
            Self::record_edit_state_for_record(&after.into())
        })
    }

    pub(crate) fn save_requirement_resource(
        &self,
        input: SaveRequirement,
    ) -> anyhow::Result<super::RequirementResourceSnapshot> {
        self.save_requirement_with_origin(input, None, |store, after| {
            store.requirement_resource_snapshot_unlocked(&after.scope_id, &after.id)
        })
    }

    /// Saves a Requirement edit as a Discussion outcome. The first history
    /// version that contains the outcome shows its Discussion origin.
    #[rule("rule_discussion_outcome_shows_record_change")]
    pub fn save_requirement_from_discussion(
        &self,
        input: SaveRequirement,
        origin: &DiscussionOrigin,
    ) -> anyhow::Result<RequirementEditState> {
        self.save_requirement_with_origin(input, Some(origin), |_, after| {
            Self::record_edit_state_for_record(&after.into())
        })
    }

    fn save_requirement_with_origin<R>(
        &self,
        mut input: SaveRequirement,
        origin: Option<&DiscussionOrigin>,
        complete: impl FnOnce(&Self, Requirement) -> anyhow::Result<R>,
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
            if let Some(origin) = origin {
                self.validate_discussion_origin(scope, origin)?;
            }
            let current_etag = etag(&ReviewRecord::from(record.clone()))?;
            let stale = (input.expected_etag != current_etag).then_some(current_etag);
            let stamp = self.current_record_stamp()?;
            with_record_state(&self.layout, |layout| {
                let staged = Self::staged(layout.clone(), stamp);
                let path = shards::requirements_path(layout, scope);
                let record_id = record.id.clone();
                guard::with_writer(&path, record_id.as_str(), || {
                    let after = staged.commit_requirement(input, &record, origin, stale)?;
                    complete(&staged, after)
                })
            })
        })
    }

    /// Applies one save. A save that would leave the record as it is returns
    /// it unchanged, even with a stale etag; any other save with a stale etag
    /// is refused.
    fn commit_requirement(
        &self,
        input: SaveRequirement,
        before: &Requirement,
        origin: Option<&DiscussionOrigin>,
        stale: Option<String>,
    ) -> anyhow::Result<Requirement> {
        let scope = before.scope_id.clone();
        let id = before.id.clone();
        let actor = input.actor;
        self.apply_requirement_update(input.update)?;
        if let Some(relationships) = input.relationships {
            crate::test_probes::at("requirement_relationships_expanding")?;
            self.replace_review_relationships(&scope, &id, &relationships)?;
        }
        let after = self.requirement(&scope, &id)?;
        self.validate_graph_scope(&scope)?;
        let before_record = ReviewRecord::from(before.clone());
        let after_record = ReviewRecord::from(after.clone());
        let content = provenance_core::model::record_stamps::content_value;
        if content(&before_record)? == content(&after_record)? {
            return Ok(before.clone());
        }
        if let Some(current_etag) = stale {
            return Err(SourceFailure::wrap(
                WriteFailure::RequirementEditConflict { current_etag },
                anyhow::anyhow!("stale Requirement edit etag"),
            ));
        }
        let fields =
            classifier::changed_fields(NodeType::Requirement, &before_record, &after_record)?;
        let revision = classifier::review_revision(NodeType::Requirement, &after_record)?;
        if let Some(origin) = origin {
            self.add_discussion_outcome(&scope, origin, NodeType::Requirement, &id, &revision)?;
        }
        if classifier::changes_revision(NodeType::Requirement, &fields) {
            self.commit_automatic_submission(&after, &actor, &revision)?;
        }
        Ok(after)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::ProvenanceLayout;
    use camino::Utf8Path;
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

    fn enroll(store: &StateStore, id: &StableId, update: &serde_json::Value) {
        let input: SaveRequirement = serde_json::from_value(json!({
            "actor":"ben",
            "expected_etag":store.requirement_edit_state(&ScopeId::new("default").unwrap(), id).unwrap().etag,
            "update":update, "relationships":null
        }))
        .unwrap();
        assert!(store.save_requirement(input).unwrap().revision.is_some());
    }

    /// Implementation aid: pins that the etag ignores record stamps; no Rule
    /// names it.
    #[test]
    fn requirement_etag_ignores_record_stamps() {
        let value = json!({
            "schema_version": 2,
            "scope_id": "default",
            "id": "req_stamp",
            "statement": "The system stores records.",
            "status": "active"
        });
        let before: Requirement = serde_json::from_value(value.clone()).unwrap();
        let mut after: Requirement = serde_json::from_value(value).unwrap();
        after.created = Some(
            serde_json::from_value(json!({"commit": "a".repeat(40), "at": "2026-09-12T00:00:00Z"}))
                .unwrap(),
        );
        after.updated = Some(
            serde_json::from_value(json!({"commit": "b".repeat(40), "at": "2026-09-12T01:00:00Z"}))
                .unwrap(),
        );
        assert_eq!(etag(&before.into()).unwrap(), etag(&after.into()).unwrap());
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
            &json!({"scope_id":"default", "id":"req_unchanged"}),
        );
        assert!(store
            .requirement_decision_state(&scope, &unchanged)
            .unwrap()
            .pending
            .is_none());
    }
}
