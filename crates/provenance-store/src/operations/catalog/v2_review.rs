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
    Requirement, RequirementStatus, ScopeId, StableId,
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
    let record = store
        .list_requirements(scope)?
        .into_iter()
        .find(|record| &record.id == id)
        .ok_or(provenance_core::protocol::read_failure::ReadFailure::ResourceNotFound)?;
    Ok(RequirementResource {
        record,
        edit: store.requirement_edit_state(scope, id)?,
        decision: store.requirement_decision_state(scope, id)?,
    })
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
            let id = request.id.clone();
            let store = StateStore::new(ProvenanceLayout::new(context.root));
            store.create_review_requirement(review::CreateReviewRequirement {
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
            resource(&store, &scope, &id).map_err(Into::into)
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
            let id = request.id.clone();
            let store = StateStore::new(ProvenanceLayout::new(context.root));
            store.save_requirement(review::SaveRequirement {
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
            resource(&store, &scope, &id).map_err(Into::into)
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
decision!(
    DecideRequirementReviewV2,
    "decide-requirement-review-v2",
    review::DecideRequirementReview,
    decide_requirement_review
);
decision!(
    WithdrawRequirementReviewV2,
    "withdraw-requirement-review-v2",
    review::WithdrawRequirementReview,
    withdraw_requirement_review
);
