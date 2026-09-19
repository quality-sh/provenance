//! Resource adapters for the guarded Requirement review interface.
use super::{
    failures::ReadError, ContextKind, ExecutionNeed, ExecutionNeeds, Operation, OperationFuture,
    PreparedContext,
};
use crate::{
    layout::ProvenanceLayout,
    review,
    state_store::{
        CreateRequirementInput, RequirementClearField, StateStore, UpdateRequirementInput,
    },
    write_error::WriteError,
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

pub struct GetRequirementV2;
impl Operation for GetRequirementV2 {
    type Request = GetRequirementRequest;
    type Success = RequirementResource;
    type Failure = ReadError;
    const NAME: &'static str = "get-requirement-v2";
    const CONTEXT: ContextKind = ContextKind::Scope;
    const FAILURE_STATUSES: &'static [u16] = &[409];
    fn needs(_: &Self::Request) -> ExecutionNeeds {
        &[ExecutionNeed::GraphStorage]
    }
    fn failure_status(error: &ReadError) -> u16 {
        error.status()
    }
    fn run(
        context: PreparedContext,
        request: Self::Request,
    ) -> OperationFuture<Self::Success, Self::Failure> {
        Box::pin(async move {
            let context = context.scope()?;
            resource(
                &StateStore::new(ProvenanceLayout::new(context.root)),
                &context.scope,
                &request.id,
            )
            .map_err(Into::into)
        })
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

pub struct CreateRequirementV2;
impl Operation for CreateRequirementV2 {
    type Request = CreateRequirementRequest;
    type Success = RequirementResource;
    type Failure = WriteError;
    const NAME: &'static str = "create-requirement-v2";
    const MUTATES: bool = true;
    const CONTEXT: ContextKind = ContextKind::Scope;
    const FAILURE_STATUSES: &'static [u16] = &[409];
    fn needs(_: &Self::Request) -> ExecutionNeeds {
        &[ExecutionNeed::GraphStorage, ExecutionNeed::Dictionary]
    }
    fn failure_status(error: &WriteError) -> u16 {
        error.status()
    }
    fn run(
        context: PreparedContext,
        request: Self::Request,
    ) -> OperationFuture<Self::Success, Self::Failure> {
        Box::pin(async move {
            let context = context.scope()?;
            let scope = context.scope;
            let store = StateStore::new(ProvenanceLayout::new(context.root));
            let snapshot = store.create_review_requirement_resource(review::CreateReviewRequirement {
                request_id: request.request_id,
                actor: request.actor,
                origin: request.origin,
                create: CreateRequirementInput {
                    scope_id: scope.clone(),
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
            })?;
            Ok(resource_from(snapshot))
        })
    }
}

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

pub struct UpdateRequirementV2;
impl Operation for UpdateRequirementV2 {
    type Request = UpdateRequirementRequest;
    type Success = RequirementResource;
    type Failure = WriteError;
    const NAME: &'static str = "update-requirement-v2";
    const MUTATES: bool = true;
    const CONTEXT: ContextKind = ContextKind::Scope;
    const FAILURE_STATUSES: &'static [u16] = &[409];
    fn needs(_: &Self::Request) -> ExecutionNeeds {
        &[ExecutionNeed::GraphStorage, ExecutionNeed::Dictionary]
    }
    fn failure_status(error: &WriteError) -> u16 {
        error.status()
    }
    fn run(
        context: PreparedContext,
        request: Self::Request,
    ) -> OperationFuture<Self::Success, Self::Failure> {
        Box::pin(async move {
            let context = context.scope()?;
            let scope = context.scope;
            let store = StateStore::new(ProvenanceLayout::new(context.root));
            let snapshot = store.save_requirement_resource(review::SaveRequirement {
                request_id: request.request_id,
                actor: request.actor,
                expected_etag: request.expected_etag,
                relationships: request.relationships,
                update: UpdateRequirementInput {
                    scope_id: scope.clone(),
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
        })
    }
}

macro_rules! decision {
    ($name:ident, $wire:literal, $request:ty, $method:ident) => {
        pub struct $name;
        impl Operation for $name {
            type Request = $request;
            type Success = CycleEntry;
            type Failure = WriteError;
            const NAME: &'static str = $wire;
            const MUTATES: bool = true;
            const CONTEXT: ContextKind = ContextKind::Scope;
            const FAILURE_STATUSES: &'static [u16] = &[409];
            fn needs(_: &Self::Request) -> ExecutionNeeds {
                &[ExecutionNeed::GraphStorage]
            }
            fn failure_status(error: &WriteError) -> u16 {
                error.status()
            }
            fn run(
                context: PreparedContext,
                request: Self::Request,
            ) -> OperationFuture<Self::Success, Self::Failure> {
                Box::pin(async move {
                    let context = context.scope()?;
                    Ok(StateStore::new(ProvenanceLayout::new(context.root)).$method(request)?)
                })
            }
        }
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
        pub struct $name;
        impl Operation for $name {
            type Request = $request;
            type Success = CycleEntry;
            type Failure = WriteError;
            const NAME: &'static str = $wire;
            const MUTATES: bool = true;
            const CONTEXT: ContextKind = ContextKind::Scope;
            const FAILURE_STATUSES: &'static [u16] = &[409];
            fn needs(_: &Self::Request) -> ExecutionNeeds {
                &[ExecutionNeed::GraphStorage]
            }
            fn failure_status(error: &WriteError) -> u16 {
                error.status()
            }
            fn run(
                context: PreparedContext,
                request: Self::Request,
            ) -> OperationFuture<Self::Success, Self::Failure> {
                Box::pin(async move {
                    let context = context.scope()?;
                    let requirement_id = request.requirement_id.clone();
                    let input: $input = ($convert)(request);
                    Ok(StateStore::new(ProvenanceLayout::new(context.root))
                        .$method(&requirement_id, input)?)
                })
            }
        }
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
