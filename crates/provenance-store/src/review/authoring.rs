//! Legacy Requirement authoring routed through guarded review writes.

use super::{
    input::{CitesEdit, ListEdit, RequirementRelations, SaveRequirement, SingleEdit},
    CreateReviewRequirement,
};
use crate::{
    canonical_digest,
    state_store::{
        AddSourceReferenceInput, CreateRequirementInput, StateStore, UpdateRequirementInput,
    },
    write_error::{SourceFailure, WriteError, WriteFailure},
};
use provenance_core::{Requirement, ScopeId, SourceReference, StableId};

const AUTHORING_ACTOR: &str = "authoring";

impl StateStore {
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
            Ok(entry) => self.requirement(&entry.scope_id, &entry.requirement_id),
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
