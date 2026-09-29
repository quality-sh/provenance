//! Legacy Requirement authoring routed through guarded review writes.

use super::{
    input::{CitesEdit, ListEdit, RequirementRelations, SaveRequirement, SingleEdit},
    CreateReviewRequirement,
};
use crate::{
    canonical_digest,
    publication::with_staged_state,
    review::{guard, journal, save::RecordEvidenceContext},
    state_store::{
        record_stamps::GraphRecord, AddSourceReferenceInput, CreateRequirementInput, StateStore,
        UpdateRequirementInput,
    },
    write_error::{SourceFailure, WriteError, WriteFailure},
};
use camino::Utf8Path;
use provenance_core::{review::ReviewRecord, Requirement, ScopeId, SourceReference, StableId};

const AUTHORING_ACTOR: &str = "authoring";

impl StateStore {
    pub(crate) fn replace_native_records<T: GraphRecord>(
        &self,
        path: &Utf8Path,
        mut replacement: Vec<T>,
    ) -> anyhow::Result<()> {
        let relative = path.strip_prefix(self.layout.root())?.to_owned();
        self.with_repository_publication(|| {
            with_staged_state(&self.layout, false, |layout| {
                let staged = Self::new(layout.clone());
                let staged_path = layout.root().join(&relative);
                guard::with_writer(&staged_path, "*", || {
                    for record in &mut replacement {
                        record.set_review_schema_version(
                            provenance_core::review::REVIEW_SCHEMA_VERSION,
                        );
                    }
                    let scope = replacement.first().map(|record| {
                        let record: ReviewRecord = record.clone().into();
                        record.scope_id().clone()
                    });
                    let before =
                        staged.replace_graph_records_guarded(&staged_path, replacement.clone())?;
                    for record in &before {
                        let review: ReviewRecord = record.clone().into();
                        anyhow::ensure!(
                            review.schema_version()
                                != provenance_core::review::REVIEW_SCHEMA_VERSION
                                || replacement.iter().any(|after| after.id() == record.id()),
                            "an enrolled graph record cannot be removed by replacement"
                        );
                    }
                    for record in replacement {
                        let previous = before.iter().find(|before| before.id() == record.id());
                        let before = previous.cloned().map(Into::into);
                        let after: ReviewRecord = record.into();
                        staged.commit_native_occurrence(before.as_ref(), &after)?;
                    }
                    if let Some(scope) = scope {
                        staged.validate_graph_scope(&scope)?;
                        staged.enroll_review_manifest()?;
                    }
                    Ok(())
                })
            })
        })
    }

    pub(crate) fn save_native_record<T: GraphRecord>(
        &self,
        path: &Utf8Path,
        expected_etag: Option<&str>,
        mutate: impl FnOnce(&mut Vec<T>) -> anyhow::Result<T>,
    ) -> anyhow::Result<T> {
        let relative = path.strip_prefix(self.layout.root())?.to_owned();
        self.with_repository_publication(|| {
            with_staged_state(&self.layout, false, |layout| {
                let staged = Self::new(layout.clone());
                let staged_path = layout.root().join(&relative);
                guard::with_writer(&staged_path, "*", || {
                    let (before, after) =
                        staged.mutate_graph_record_guarded(&staged_path, mutate)?;
                    let before = before.ok_or_else(|| {
                        anyhow::anyhow!("native update cannot create a graph record")
                    })?;
                    let before: ReviewRecord = before.into();
                    if let Some(expected_etag) = expected_etag {
                        let head = staged.head(&before)?;
                        let current_etag = head
                            .as_ref()
                            .map(|entry| entry.etag.clone())
                            .unwrap_or(journal::etag(&before, None)?);
                        if expected_etag != current_etag {
                            return Err(SourceFailure::wrap(
                                WriteFailure::RequirementEditConflict { current_etag },
                                anyhow::anyhow!("stale native record edit etag"),
                            ));
                        }
                    }
                    let after = if native_record_is_closed(&staged_path, T::KIND, after.id())? {
                        staged.enroll_graph_record::<T>(&staged_path, after.id())?
                    } else {
                        after
                    };
                    let after: ReviewRecord = after.into();
                    staged.commit_native_occurrence(Some(&before), &after)?;
                    staged.validate_graph_scope(after.scope_id())?;
                    staged.enroll_review_manifest()?;
                    Ok(after)
                })
            })
        })
        .and_then(review_record_into)
    }

    fn commit_native_occurrence(
        &self,
        before: Option<&ReviewRecord>,
        after: &ReviewRecord,
    ) -> anyhow::Result<()> {
        let head = before
            .map(|record| self.head(record))
            .transpose()?
            .flatten();
        self.validated_review_entries(after.scope_id())?;
        let request_id = journal::new_id();
        self.commit_record_evidence(
            before,
            after,
            RecordEvidenceContext {
                head,
                actor: AUTHORING_ACTOR.to_owned(),
                request_id,
                intent_digest: canonical_digest::digest(&canonical_digest::canonical_bytes(after)?),
                origin: None,
            },
        )?;
        Ok(())
    }

    pub(crate) fn create_native_record<T: GraphRecord>(
        &self,
        path: &Utf8Path,
        id: &StableId,
        write: impl FnOnce(&Self) -> anyhow::Result<T>,
    ) -> anyhow::Result<T> {
        let relative = path.strip_prefix(self.layout.root())?.to_owned();
        self.with_repository_publication(|| {
            with_staged_state(&self.layout, false, |layout| {
                let staged = Self::new(layout.clone());
                let staged_path = layout.root().join(&relative);
                guard::with_writer(&staged_path, id.as_str(), || {
                    write(&staged)?;
                    let created = staged.enroll_graph_record::<T>(&staged_path, id)?;
                    let after: ReviewRecord = created.clone().into();
                    let request_id = journal::new_id();
                    staged.commit_record_evidence(
                        None,
                        &after,
                        RecordEvidenceContext {
                            head: None,
                            actor: AUTHORING_ACTOR.to_owned(),
                            request_id,
                            intent_digest: canonical_digest::digest(
                                &canonical_digest::canonical_bytes(&after)?,
                            ),
                            origin: None,
                        },
                    )?;
                    staged.enroll_review_manifest()?;
                    Ok(created)
                })
            })
        })
    }

    pub fn create_requirement(&self, input: CreateRequirementInput) -> anyhow::Result<Requirement> {
        let intent = intent_of(&input)?;
        let request_id = authoring_request_id("create-requirement", &[&intent])?;
        let scope = input.scope_id.clone();
        let id = input.id.clone();
        match self.create_review_requirement(CreateReviewRequirement {
            request_id,
            actor: AUTHORING_ACTOR.to_owned(),
            create: input,
            origin: None,
        }) {
            Ok(entry) => self.requirement(&entry.scope_id, &entry.record_id),
            Err(error) => {
                let duplicate = self
                    .list_requirements(&scope)
                    .is_ok_and(|records| records.iter().any(|record| record.id == id));
                Err(retype_create_error(error, duplicate))
            }
        }
    }

    pub fn update_requirement(&self, input: UpdateRequirementInput) -> anyhow::Result<Requirement> {
        self.save_record(input, None)
    }

    /// Sets or clears the deliberately unstructured fog text.
    pub fn set_requirement_fog(
        &self,
        scope_id: &ScopeId,
        id: &StableId,
        fog: Option<String>,
    ) -> anyhow::Result<Requirement> {
        let mut update = empty_update(scope_id, id);
        match fog {
            Some(fog) => {
                anyhow::ensure!(!fog.trim().is_empty(), "fog text must not be empty");
                update.fog = Some(fog);
            }
            None => update.clear_fields = vec![crate::state_store::RequirementClearField::Fog],
        }
        self.save_record(update, None)
    }

    pub fn add_source_reference(
        &self,
        input: AddSourceReferenceInput,
    ) -> anyhow::Result<Requirement> {
        let AddSourceReferenceInput {
            scope_id,
            source_id,
            requirement_id,
            clause,
        } = input;
        self.save_record(
            empty_update(&scope_id, &requirement_id),
            Some(RequirementRelations {
                cites: Some(CitesEdit::Delta {
                    add: vec![SourceReference { source_id, clause }],
                    remove: Vec::new(),
                }),
                ..RequirementRelations::default()
            }),
        )
    }

    pub fn set_requirement_refines(
        &self,
        scope_id: &ScopeId,
        requirement: &StableId,
        target: StableId,
    ) -> anyhow::Result<Requirement> {
        self.save_record(
            empty_update(scope_id, requirement),
            Some(RequirementRelations {
                refines: Some(SingleEdit::Set(target)),
                ..RequirementRelations::default()
            }),
        )
    }

    pub fn clear_requirement_refines(
        &self,
        scope_id: &ScopeId,
        requirement: &StableId,
    ) -> anyhow::Result<Requirement> {
        self.save_record(
            empty_update(scope_id, requirement),
            Some(RequirementRelations {
                refines: Some(SingleEdit::Clear),
                ..RequirementRelations::default()
            }),
        )
    }

    pub fn add_requirement_depends_on(
        &self,
        scope_id: &ScopeId,
        requirement: &StableId,
        target: StableId,
    ) -> anyhow::Result<Requirement> {
        self.save_record(
            empty_update(scope_id, requirement),
            Some(RequirementRelations {
                depends_on: Some(add_delta(vec![target])),
                ..RequirementRelations::default()
            }),
        )
    }

    pub fn clear_requirement_depends_on(
        &self,
        scope_id: &ScopeId,
        requirement: &StableId,
        target: &StableId,
    ) -> anyhow::Result<Requirement> {
        self.save_record(
            empty_update(scope_id, requirement),
            Some(RequirementRelations {
                depends_on: Some(remove_delta(vec![target.clone()])),
                ..RequirementRelations::default()
            }),
        )
    }

    pub fn add_requirement_supersedes(
        &self,
        scope_id: &ScopeId,
        requirement: &StableId,
        target: StableId,
    ) -> anyhow::Result<Requirement> {
        self.save_record(
            empty_update(scope_id, requirement),
            Some(RequirementRelations {
                supersedes: Some(add_delta(vec![target])),
                ..RequirementRelations::default()
            }),
        )
    }

    pub fn clear_requirement_supersedes(
        &self,
        scope_id: &ScopeId,
        requirement: &StableId,
        target: &StableId,
    ) -> anyhow::Result<Requirement> {
        self.save_record(
            empty_update(scope_id, requirement),
            Some(RequirementRelations {
                supersedes: Some(remove_delta(vec![target.clone()])),
                ..RequirementRelations::default()
            }),
        )
    }

    pub fn set_requirement_spawned_by(
        &self,
        scope_id: &ScopeId,
        requirement: &StableId,
        target: StableId,
    ) -> anyhow::Result<Requirement> {
        self.save_record(
            empty_update(scope_id, requirement),
            Some(RequirementRelations {
                spawned_by: Some(SingleEdit::Set(target)),
                ..RequirementRelations::default()
            }),
        )
    }

    pub fn clear_requirement_spawned_by(
        &self,
        scope_id: &ScopeId,
        requirement: &StableId,
    ) -> anyhow::Result<Requirement> {
        self.save_record(
            empty_update(scope_id, requirement),
            Some(RequirementRelations {
                spawned_by: Some(SingleEdit::Clear),
                ..RequirementRelations::default()
            }),
        )
    }

    /// Removes every citation of one source from a Requirement.
    pub fn clear_source_reference(
        &self,
        scope_id: &ScopeId,
        requirement: &StableId,
        source: &StableId,
    ) -> anyhow::Result<Requirement> {
        self.save_record(
            empty_update(scope_id, requirement),
            Some(RequirementRelations {
                cites: Some(CitesEdit::Delta {
                    add: Vec::new(),
                    remove: vec![source.clone()],
                }),
                ..RequirementRelations::default()
            }),
        )
    }

    fn save_record(
        &self,
        update: UpdateRequirementInput,
        relationships: Option<RequirementRelations>,
    ) -> anyhow::Result<Requirement> {
        let scope = update.scope_id.clone();
        let id = update.id.clone();
        let expected_etag = self.requirement_edit_state(&scope, &id)?.etag;
        let intent = intent_of(&(&update, &relationships))?;
        let request_id =
            authoring_request_id("update-requirement", &[&intent, expected_etag.as_str()])?;
        self.save_requirement(SaveRequirement {
            request_id,
            actor: AUTHORING_ACTOR.to_owned(),
            expected_etag,
            update,
            relationships,
        })?;
        self.requirement(&scope, &id)
    }
}

fn review_record_into<T>(record: ReviewRecord) -> anyhow::Result<T>
where
    T: serde::de::DeserializeOwned,
{
    Ok(serde_json::from_value(serde_json::to_value(record)?)?)
}

fn native_record_is_closed(
    path: &Utf8Path,
    kind: provenance_core::NodeType,
    id: &StableId,
) -> anyhow::Result<bool> {
    let contents = std::fs::read_to_string(path)?;
    let mut matched = None;
    for line in contents.lines() {
        let value: serde_json::Value = serde_json::from_str(line)?;
        if value["id"].as_str() == Some(id.as_str()) {
            matched = Some(value);
            break;
        }
    }
    let value = matched.ok_or_else(|| anyhow::anyhow!("updated graph record is missing"))?;
    Ok(provenance_core::review::ReviewRecord::deserialize_closed(kind, &value).is_ok())
}

fn retype_create_error(error: anyhow::Error, duplicate: bool) -> anyhow::Error {
    if !duplicate {
        return error;
    }
    let wrapper = WriteError(error);
    if matches!(wrapper.safe(), WriteFailure::WriteFailed) {
        return SourceFailure::wrap(
            WriteFailure::AlreadyExists,
            anyhow::anyhow!("requirement already exists"),
        );
    }
    wrapper.0
}

const fn add_delta(add: Vec<StableId>) -> ListEdit {
    ListEdit::Delta {
        add,
        remove: Vec::new(),
    }
}

const fn remove_delta(remove: Vec<StableId>) -> ListEdit {
    ListEdit::Delta {
        add: Vec::new(),
        remove,
    }
}

fn empty_update(scope_id: &ScopeId, id: &StableId) -> UpdateRequirementInput {
    UpdateRequirementInput {
        scope_id: scope_id.clone(),
        id: id.clone(),
        expected_etag: None,
        declared_by: None,
        statement: None,
        description: None,
        fog: None,
        status: None,
        domain_id: None,
        clear_fields: Vec::new(),
    }
}

fn intent_of(value: &impl serde::Serialize) -> anyhow::Result<String> {
    Ok(canonical_digest::digest(
        &canonical_digest::canonical_bytes(value)?,
    ))
}

fn authoring_request_id(label: &str, parts: &[&str]) -> anyhow::Result<StableId> {
    let joined = format!("{label}\u{1f}{}", parts.join("\u{1f}"));
    StableId::new(canonical_digest::sha256(joined.as_bytes()))
}
