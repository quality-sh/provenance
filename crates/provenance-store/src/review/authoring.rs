//! Legacy Requirement authoring routed through guarded review writes.

use super::{
    input::{CitesEdit, ListEdit, RequirementRelations, SaveRequirement, SingleEdit},
    CreateReviewRequirement,
};
use crate::{
    review::guard,
    review::publication::with_record_state,
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
        replacement: Vec<T>,
    ) -> anyhow::Result<()> {
        let relative = path.strip_prefix(self.layout.root())?.to_owned();
        let stamp = self.current_record_stamp()?;
        self.with_repository_publication(|| {
            with_record_state(&self.layout, |layout| {
                let staged = Self::staged(layout.clone(), stamp.clone());
                let staged_path = layout.root().join(&relative);
                guard::with_writer(&staged_path, "*", || {
                    let scope = replacement.first().map(|record| {
                        let record: ReviewRecord = record.clone().into();
                        record.scope_id().clone()
                    });
                    let before =
                        staged.replace_graph_records_guarded(&staged_path, replacement.clone())?;
                    for record in &before {
                        anyhow::ensure!(
                            replacement.iter().any(|after| after.id() == record.id()),
                            "an enrolled graph record cannot be removed by replacement"
                        );
                    }
                    if let Some(scope) = scope {
                        staged.validate_graph_scope(&scope)?;
                    }
                    Ok(())
                })
            })
        })
    }

    pub(crate) fn create_native_record<T: GraphRecord>(
        &self,
        path: &Utf8Path,
        id: &StableId,
        write: impl FnOnce(&Self) -> anyhow::Result<T>,
    ) -> anyhow::Result<T> {
        let relative = path.strip_prefix(self.layout.root())?.to_owned();
        let stamp = self.current_record_stamp()?;
        self.with_repository_publication(|| {
            with_record_state(&self.layout, |layout| {
                let staged = Self::staged(layout.clone(), stamp.clone());
                let staged_path = layout.root().join(&relative);
                guard::with_writer(&staged_path, id.as_str(), || write(&staged))
            })
        })
    }

    /// Creates a Requirement, or returns the stored one when it already holds
    /// exactly the requested content.
    pub fn create_requirement(&self, input: CreateRequirementInput) -> anyhow::Result<Requirement> {
        let scope = input.scope_id.clone();
        let id = input.id.clone();
        if let Some(existing) = self.equal_creation(&input)? {
            return Ok(existing);
        }
        match self.create_review_requirement(CreateReviewRequirement {
            actor: AUTHORING_ACTOR.to_owned(),
            create: input,
            origin: None,
        }) {
            Ok(_) => self.requirement(&scope, &id),
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
        self.save_requirement(SaveRequirement {
            actor: AUTHORING_ACTOR.to_owned(),
            expected_etag,
            update,
            relationships,
        })?;
        self.requirement(&scope, &id)
    }
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

impl StateStore {
    /// The stored Requirement when it already holds every requested value.
    fn equal_creation(
        &self,
        input: &CreateRequirementInput,
    ) -> anyhow::Result<Option<Requirement>> {
        let Some(existing) = self
            .list_requirements(&input.scope_id)?
            .into_iter()
            .find(|record| record.id == input.id)
        else {
            return Ok(None);
        };
        let mut wanted = serde_json::to_value(input)?;
        for field in ["depends_on", "supersedes"] {
            if let Some(serde_json::Value::Array(ids)) = wanted.get_mut(field) {
                ids.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
                ids.dedup();
            }
        }
        let stored = serde_json::to_value(&existing)?;
        let absent = |value: &serde_json::Value| {
            value.is_null() || value.as_array().is_some_and(Vec::is_empty)
        };
        let equal = wanted.as_object().is_some_and(|fields| {
            fields.iter().all(|(name, value)| {
                stored.get(name).map_or_else(
                    || absent(value),
                    |current| current == value || (absent(current) && absent(value)),
                )
            })
        });
        Ok(equal.then_some(existing))
    }
}
