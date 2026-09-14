//! The legacy authoring surfaces, kept until Phase 2b re-registers the
//! catalog.
//!
//! The v2 contract gives Requirements one guarded write path: the journaled
//! review operations. Every create and edit below routes through that path, so
//! it enrolls actor identity, request identity, edit preconditions, and the
//! before/after journal entries. No bridge here writes a record directly.
//!
//! The legacy surface carries no actor or idempotency facts, so each bridge
//! derives them deterministically: the actor is the authoring identity and the
//! request identity is the digest of the call and the edit precondition it was
//! computed against. A repeated identical call therefore resolves to the same
//! journal receipt, and a changed record state produces a new identity instead
//! of a false intent clash.

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

/// The actor a legacy authoring call records, which carries no actor fact.
const AUTHORING_ACTOR: &str = "authoring";

impl StateStore {
    /// LEGACY (Phase 2b re-registers this surface): creates a Requirement
    /// through the guarded journal creation.
    pub fn create_requirement(&self, input: CreateRequirementInput) -> anyhow::Result<Requirement> {
        let request_id = authoring_request_id("create-requirement", &[&intent_of(&input)?])?;
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
                Err(retyped(error, duplicate))
            }
        }
    }

    /// LEGACY (Phase 2b re-registers this surface): edits a Requirement
    /// through the guarded journal save under the current edit precondition.
    pub fn update_requirement(&self, input: UpdateRequirementInput) -> anyhow::Result<Requirement> {
        self.save_record(input, None)
    }

    /// LEGACY (Phase 2b re-registers this surface): sets or clears the
    /// deliberately unstructured fog text through the guarded journal save.
    pub fn set_requirement_fog(
        &self,
        scope_id: &ScopeId,
        id: &StableId,
        fog: Option<String>,
    ) -> anyhow::Result<Requirement> {
        if let Some(fog) = &fog {
            anyhow::ensure!(!fog.trim().is_empty(), "fog text must not be empty");
        }
        let mut update = empty_update(scope_id, id);
        if fog.is_some() {
            update.fog = fog;
        } else {
            update.clear_fields = vec![crate::state_store::RequirementClearField::Fog];
        }
        self.save_record(update, None)
    }

    /// LEGACY (Phase 2b re-registers this surface): adds one citation through
    /// the guarded journal save.
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

    /// LEGACY (Phase 2b re-registers this surface).
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

    /// LEGACY (Phase 2b re-registers this surface).
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

    /// LEGACY (Phase 2b re-registers this surface).
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

    /// LEGACY (Phase 2b re-registers this surface).
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

    /// LEGACY (Phase 2b re-registers this surface).
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

    /// LEGACY (Phase 2b re-registers this surface).
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

    /// LEGACY (Phase 2b re-registers this surface).
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

    /// LEGACY (Phase 2b re-registers this surface).
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

    /// LEGACY (Phase 2b re-registers this surface): removes every clause that
    /// cites one source through the guarded journal save.
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

    /// Publishes one guarded save under the authoring identity and the current
    /// edit precondition, then returns the resulting record.
    fn save_record(
        &self,
        update: UpdateRequirementInput,
        relationships: Option<RequirementRelations>,
    ) -> anyhow::Result<Requirement> {
        let scope = update.scope_id.clone();
        let id = update.id.clone();
        let expected_etag = self.requirement_edit_state(&scope, &id)?.etag;
        let request_id = authoring_request_id(
            "update-requirement",
            &[
                &intent_of(&(&update, &relationships))?,
                expected_etag.as_str(),
            ],
        )?;
        // The guarded save types its own refusals: a validation refusal is
        // InvalidUpdate from its raise site, and an infrastructure failure
        // stays WriteFailed.
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

/// Keeps every failure with the class its raise site gave it. Only the
/// create path's duplicate resolution stays here: a create refusal while the
/// scope already holds the identity is the AlreadyExists class, and an
/// infrastructure failure keeps the 500 WriteFailed class.
fn retyped(error: anyhow::Error, duplicate: bool) -> anyhow::Error {
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

/// A text update that changes nothing.
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

/// The stable request identity of one legacy authoring call.
fn authoring_request_id(label: &str, parts: &[&str]) -> anyhow::Result<StableId> {
    let joined = format!("{label}\u{1f}{}", parts.join("\u{1f}"));
    StableId::new(canonical_digest::sha256(joined.as_bytes()))
}
