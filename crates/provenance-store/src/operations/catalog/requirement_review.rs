//! Resource adapters for the guarded Requirement review interface.
use super::{
    shapes::scoped_write_operation,
    ExecutionNeed,
};
use crate::{
    review,
    state_store::{
        CreateRequirementInput, RequirementClearField, StateStore, UpdateRequirementInput,
    },
};
use provenance_core::{
    review::{CycleEntry, RequirementDecisionState, RequirementEditState},
    CanonicalArtifact, DispositionActor, DispositionDecision, Requirement, RequirementStatus,
    ScopeId, StableId,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct RequirementResource {
    #[serde(flatten)]
    pub record: Requirement,
    pub edit: RequirementEditState,
    pub decision: RequirementDecisionState,
}

fn resource_from(snapshot: review::RequirementResourceSnapshot) -> RequirementResource {
    RequirementResource {
        record: snapshot.record,
        edit: snapshot.edit,
        decision: snapshot.decision,
    }
}

#[derive(Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CreateRequirementRequest {
    pub request_id: StableId,
    pub actor: String,
    pub id: StableId,
    pub statement: String,
    pub description: Option<String>,
    pub status: RequirementStatus,
    pub domain_id: Option<StableId>,
    pub refines: Option<StableId>,
    pub depends_on: Vec<StableId>,
    pub supersedes: Vec<StableId>,
    pub spawned_by: Option<StableId>,
    pub origin_thread: Option<StableId>,
    pub origin_message: Option<StableId>,
    pub origin: Option<provenance_core::threads::DiscussionOrigin>,
}

scoped_write_operation!(
    pub CreateRequirementResource,
    "create-requirement",
    CreateRequirementRequest,
    RequirementResource,
    &[409],
    &[ExecutionNeed::GraphStorage, ExecutionNeed::Dictionary],
    scope = none,
    |store, scope, request| {
        let snapshot = store.create_review_requirement_resource(
            review::CreateReviewRequirement {
                request_id: request.request_id,
                actor: request.actor,
                origin: request.origin,
                create: CreateRequirementInput {
                    scope_id: scope,
                    id: request.id,
                    statement: request.statement,
                    description: request.description,
                    status: request.status,
                    domain_id: request.domain_id,
                    refines: request.refines,
                    depends_on: request.depends_on,
                    supersedes: request.supersedes,
                    spawned_by: request.spawned_by,
                    origin_thread: request.origin_thread,
                    origin_message: request.origin_message,
                },
            },
        )?;
        Ok(resource_from(snapshot))
    }
);

#[derive(Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct UpdateRequirementRequest {
    pub request_id: StableId,
    pub actor: String,
    pub expected_etag: String,
    pub declared_by: Option<String>,
    pub statement: Option<String>,
    pub description: Option<String>,
    pub fog: Option<String>,
    pub status: Option<RequirementStatus>,
    pub domain_id: Option<StableId>,
    #[serde(default)]
    pub clear_fields: Vec<RequirementClearField>,
    pub relationships: Option<review::RequirementRelations>,
    pub id: StableId,
}

scoped_write_operation!(
    pub UpdateRequirementResource,
    "update-requirement",
    UpdateRequirementRequest,
    RequirementResource,
    &[409],
    &[ExecutionNeed::GraphStorage, ExecutionNeed::Dictionary],
    scope = none,
    |store, scope, request| {
        let snapshot = store.save_requirement_resource(review::SaveRequirement {
                request_id: request.request_id,
                actor: request.actor,
                expected_etag: request.expected_etag,
                relationships: request.relationships,
                update: UpdateRequirementInput {
                    scope_id: scope,
                    id: request.id,
                    declared_by: request.declared_by,
                    statement: request.statement,
                    description: request.description,
                    fog: request.fog,
                    status: request.status,
                    domain_id: request.domain_id,
                    clear_fields: request.clear_fields,
                },
        })?;
        Ok(resource_from(snapshot))
    }
);

macro_rules! decision {
    ($name:ident, $wire:literal, $request:ty, $method:ident) => {
        scoped_write_operation!(
            pub $name, $wire, $request, CycleEntry, &[409], &[ExecutionNeed::GraphStorage],
            scope = scope_id,
            |store, _scope, request| store.$method(request)
        );
    };
}
decision!(
    SubmitRequirementReview,
    "submit-requirement-review",
    review::SubmitRequirementReview,
    submit_record_review
);

#[derive(Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DecideRequirementReviewRequest {
    pub scope_id: ScopeId,
    #[serde(default = "requirement_kind")]
    pub record_kind: provenance_core::NodeType,
    #[serde(alias = "requirement_id")]
    pub record_id: StableId,
    pub actor: DispositionActor,
    pub proposal_id: StableId,
    pub decision: DispositionDecision,
    pub rationale: Option<String>,
    pub canonical_artifact: Option<CanonicalArtifact>,
    pub feedback: Option<review::ReviewFeedback>,
    pub declared_by: Option<String>,
}

#[derive(Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct WithdrawRequirementReviewRequest {
    pub scope_id: ScopeId,
    #[serde(default = "requirement_kind")]
    pub record_kind: provenance_core::NodeType,
    #[serde(alias = "requirement_id")]
    pub record_id: StableId,
    pub actor: String,
    pub proposal_id: StableId,
    pub declared_by: Option<String>,
    pub reason: Option<String>,
}

macro_rules! addressed_decision {
    ($name:ident, $wire:literal, $request:ty, $input:ty, $method:ident, $convert:expr) => {
        scoped_write_operation!(
            pub $name, $wire, $request, CycleEntry, &[409], &[ExecutionNeed::GraphStorage],
            scope = scope_id,
            |store, _scope, request| {
                let record_kind = request.record_kind;
                let record_id = request.record_id.clone();
                let input: $input = ($convert)(request);
                store.$method(record_kind, &record_id, input)
            }
        );
    };
}

addressed_decision!(
    DecideRequirementReview,
    "decide-requirement-review",
    DecideRequirementReviewRequest,
    review::DecideRequirementReview,
    decide_record_review_for,
    |request: DecideRequirementReviewRequest| review::DecideRequirementReview {
        scope_id: request.scope_id,
        actor: request.actor,
        proposal_id: request.proposal_id,
        decision: request.decision,
        rationale: request.rationale,
        canonical_artifact: request.canonical_artifact,
        feedback: request.feedback,
        declared_by: request.declared_by,
    }
);

#[cfg(test)]
#[path = "requirement_review_tests.rs"]
mod requirement_review_tests;
addressed_decision!(
    WithdrawRequirementReview,
    "withdraw-requirement-review",
    WithdrawRequirementReviewRequest,
    review::WithdrawRequirementReview,
    withdraw_record_review_for,
    |request: WithdrawRequirementReviewRequest| review::WithdrawRequirementReview {
        scope_id: request.scope_id,
        actor: request.actor,
        proposal_id: request.proposal_id,
        declared_by: request.declared_by,
        reason: request.reason,
    }
);

const fn requirement_kind() -> provenance_core::NodeType {
    provenance_core::NodeType::Requirement
}
