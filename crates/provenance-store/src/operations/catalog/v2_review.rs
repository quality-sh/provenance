//! Resource adapters for the guarded Requirement review interface.
use super::{
    shapes::{scoped_read_operation, scoped_write_operation},
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

fn resource(
    store: &StateStore,
    scope: &ScopeId,
    id: &StableId,
) -> anyhow::Result<RequirementResource> {
    let snapshot = store.requirement_resource_snapshot(scope, id)?;
    Ok(resource_from(snapshot))
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
pub struct GetRequirementRequest {
    pub id: StableId,
}

scoped_read_operation!(
    pub GetRequirementV2,
    "get-requirement-v2",
    GetRequirementRequest,
    RequirementResource,
    &[409],
    &[ExecutionNeed::GraphStorage],
    |store, scope, request| resource(store, scope, &request.id)
);

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
    pub CreateRequirementV2,
    "create-requirement-v2",
    CreateRequirementRequest,
    RequirementResource,
    &[409],
    &[ExecutionNeed::GraphStorage, ExecutionNeed::Dictionary],
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
    pub UpdateRequirementV2,
    "update-requirement-v2",
    UpdateRequirementRequest,
    RequirementResource,
    &[409],
    &[ExecutionNeed::GraphStorage, ExecutionNeed::Dictionary],
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
            |store, _scope, request| store.$method(request)
        );
    };
}
decision!(
    SubmitRequirementReviewV2,
    "submit-requirement-review-v2",
    review::SubmitRequirementReview,
    submit_requirement_review
);

#[derive(Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DecideRequirementReviewRequest {
    pub scope_id: ScopeId,
    pub requirement_id: StableId,
    pub request_id: StableId,
    pub actor: DispositionActor,
    pub proposal_id: StableId,
    pub disposition_id: StableId,
    pub decision: DispositionDecision,
    pub rationale: String,
    pub canonical_artifact: Option<CanonicalArtifact>,
    pub feedback: Option<review::ReviewFeedback>,
    pub declared_by: Option<String>,
}

#[derive(Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct WithdrawRequirementReviewRequest {
    pub scope_id: ScopeId,
    pub requirement_id: StableId,
    pub request_id: StableId,
    pub actor: String,
    pub proposal_id: StableId,
    pub declared_by: Option<String>,
    pub reason: Option<String>,
}

macro_rules! addressed_decision {
    ($name:ident, $wire:literal, $request:ty, $input:ty, $method:ident, $convert:expr) => {
        scoped_write_operation!(
            pub $name, $wire, $request, CycleEntry, &[409], &[ExecutionNeed::GraphStorage],
            |store, _scope, request| {
                let requirement_id = request.requirement_id.clone();
                let input: $input = ($convert)(request);
                store.$method(&requirement_id, input)
            }
        );
    };
}

addressed_decision!(
    DecideRequirementReviewV2,
    "decide-requirement-review-v2",
    DecideRequirementReviewRequest,
    review::DecideRequirementReview,
    decide_requirement_review_for,
    |request: DecideRequirementReviewRequest| review::DecideRequirementReview {
        scope_id: request.scope_id,
        request_id: request.request_id,
        actor: request.actor,
        proposal_id: request.proposal_id,
        disposition_id: request.disposition_id,
        decision: request.decision,
        rationale: request.rationale,
        canonical_artifact: request.canonical_artifact,
        feedback: request.feedback,
        declared_by: request.declared_by,
    }
);

#[cfg(test)]
#[path = "v2_review_tests.rs"]
mod v2_review_tests;
addressed_decision!(
    WithdrawRequirementReviewV2,
    "withdraw-requirement-review-v2",
    WithdrawRequirementReviewRequest,
    review::WithdrawRequirementReview,
    withdraw_requirement_review_for,
    |request: WithdrawRequirementReviewRequest| review::WithdrawRequirementReview {
        scope_id: request.scope_id,
        request_id: request.request_id,
        actor: request.actor,
        proposal_id: request.proposal_id,
        declared_by: request.declared_by,
        reason: request.reason,
    }
);
