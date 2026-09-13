//! Requirement review operations keep native inputs and results at every adapter.

use super::{
    failures::ReadError, ExecutionNeed, ExecutionNeeds, Operation, OperationFuture, PreparedContext,
};
use crate::{
    layout::ProvenanceLayout,
    review::{
        CreateReviewRequirement, DecideRequirementReview, RequirementReviewRequest,
        SaveRequirement, SubmitRequirementReview, WithdrawRequirementReview, WriteDiscussion,
    },
    state_store::StateStore,
    write_error::{SourceFailure, WriteError, WriteFailure},
};
use provenance_core::{ScopeId, StableId};
use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequirementIdRequest {
    pub requirement_id: StableId,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequirementSaveReceiptRequest {
    pub requirement_id: StableId,
    pub request_id: StableId,
    pub actor: String,
    pub declared_by: Option<String>,
}

fn store(context: PreparedContext) -> Result<(StateStore, ScopeId), WriteError> {
    let context = context.scope()?;
    Ok((
        StateStore::new(ProvenanceLayout::new(context.root)),
        context.scope,
    ))
}

fn read_store(context: PreparedContext) -> Result<(StateStore, ScopeId), ReadError> {
    let context = context.scope()?;
    Ok((
        StateStore::new(ProvenanceLayout::new(context.root)),
        context.scope,
    ))
}

fn check_scope(expected: &ScopeId, actual: &ScopeId) -> Result<(), WriteError> {
    if expected == actual {
        Ok(())
    } else {
        Err(SourceFailure::wrap(
            WriteFailure::ScopeMismatch,
            anyhow::anyhow!("request scope does not match selected scope"),
        )
        .into())
    }
}

macro_rules! write_operation {
    ($name:ident, $wire:literal, $request:ty, $success:ty, $scope:expr, $call:expr) => {
        pub struct $name;
        impl Operation for $name {
            type Request = $request;
            type Success = $success;
            type Failure = WriteError;
            const NAME: &'static str = $wire;
            const MUTATES: bool = true;
            const CONTEXT: super::ContextKind = super::ContextKind::Scope;
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
                    let (store, selected_scope) = store(context)?;
                    check_scope(&selected_scope, &($scope)(&request))?;
                    ($call)(&store, request).map_err(Into::into)
                })
            }
        }
    };
}

write_operation!(
    SaveRequirementOperation,
    "save-requirement",
    SaveRequirement,
    provenance_core::review::ReviewEntry,
    |request: &SaveRequirement| request.update.scope_id.clone(),
    |store: &StateStore, request| store.save_requirement(request)
);
write_operation!(
    CreateReviewRequirementOperation,
    "create-review-requirement",
    CreateReviewRequirement,
    provenance_core::review::ReviewEntry,
    |request: &CreateReviewRequirement| request.create.scope_id.clone(),
    |store: &StateStore, request| store.create_review_requirement(request)
);
write_operation!(
    WriteDiscussionOperation,
    "write-discussion",
    WriteDiscussion,
    provenance_core::threads::DiscussionEntry,
    |request: &WriteDiscussion| request.scope_id.clone(),
    |store: &StateStore, request| store.write_discussion(request)
);
write_operation!(
    SubmitRequirementReviewOperation,
    "submit-requirement-review",
    SubmitRequirementReview,
    provenance_core::review::CycleEntry,
    |request: &SubmitRequirementReview| request.scope_id.clone(),
    |store: &StateStore, request| store.submit_requirement_review(request)
);
write_operation!(
    DecideRequirementReviewOperation,
    "decide-requirement-review",
    DecideRequirementReview,
    provenance_core::review::CycleEntry,
    |request: &DecideRequirementReview| request.scope_id.clone(),
    |store: &StateStore, request| store.decide_requirement_review(request)
);
write_operation!(
    WithdrawRequirementReviewOperation,
    "withdraw-requirement-review",
    WithdrawRequirementReview,
    provenance_core::review::CycleEntry,
    |request: &WithdrawRequirementReview| request.scope_id.clone(),
    |store: &StateStore, request| store.withdraw_requirement_review(request)
);

macro_rules! state_read {
    ($name:ident, $wire:literal, $request:ty, $success:ty, $call:expr) => {
        pub struct $name;
        impl Operation for $name {
            type Request = $request;
            type Success = $success;
            type Failure = ReadError;
            const NAME: &'static str = $wire;
            const CONTEXT: super::ContextKind = super::ContextKind::Scope;
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
                    let (store, scope) = read_store(context)?;
                    ($call)(&store, &scope, request).map_err(Into::into)
                })
            }
        }
    };
}

state_read!(
    RequirementEditStateOperation,
    "requirement-edit-state",
    RequirementIdRequest,
    provenance_core::review::RequirementEditState,
    |store: &StateStore, scope: &ScopeId, request: RequirementIdRequest| store
        .requirement_edit_state(scope, &request.requirement_id)
);
state_read!(
    RequirementSaveReceiptOperation,
    "requirement-save-receipt",
    RequirementSaveReceiptRequest,
    Option<provenance_core::review::ReviewEntry>,
    |store: &StateStore, scope: &ScopeId, request: RequirementSaveReceiptRequest| store
        .requirement_save_receipt(
            scope,
            &request.requirement_id,
            &request.request_id,
            &request.actor,
            request.declared_by.as_deref(),
        )
);
state_read!(
    RequirementDecisionStateOperation,
    "requirement-decision-state",
    RequirementIdRequest,
    provenance_core::review::RequirementDecisionState,
    |store: &StateStore, scope: &ScopeId, request: RequirementIdRequest| store
        .requirement_decision_state(scope, &request.requirement_id)
);

macro_rules! receipt_read {
    ($name:ident, $wire:literal, $request:ty, $success:ty, $scope:expr, $call:expr) => {
        pub struct $name;
        impl Operation for $name {
            type Request = $request;
            type Success = Option<$success>;
            type Failure = WriteError;
            const NAME: &'static str = $wire;
            const CONTEXT: super::ContextKind = super::ContextKind::Scope;
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
                    let (store, selected_scope) = store(context)?;
                    check_scope(&selected_scope, &($scope)(&request))?;
                    ($call)(&store, request).map_err(Into::into)
                })
            }
        }
    };
}

receipt_read!(
    RequirementCreationReceiptOperation,
    "requirement-creation-receipt",
    CreateReviewRequirement,
    provenance_core::review::ReviewEntry,
    |request: &CreateReviewRequirement| request.create.scope_id.clone(),
    |store: &StateStore, request| store.requirement_creation_receipt(request)
);
receipt_read!(
    DiscussionReceiptOperation,
    "discussion-receipt",
    WriteDiscussion,
    provenance_core::threads::DiscussionEntry,
    |request: &WriteDiscussion| request.scope_id.clone(),
    |store: &StateStore, request| store.discussion_receipt(&request)
);
receipt_read!(
    RequirementReviewReceiptOperation,
    "requirement-review-receipt",
    RequirementReviewRequest,
    provenance_core::review::CycleEntry,
    review_request_scope,
    |store: &StateStore, request| store.requirement_review_receipt(&request)
);

fn review_request_scope(request: &RequirementReviewRequest) -> ScopeId {
    match request {
        RequirementReviewRequest::Submit { request } => request.scope_id.clone(),
        RequirementReviewRequest::Decide { request } => request.scope_id.clone(),
        RequirementReviewRequest::Withdraw { request } => request.scope_id.clone(),
    }
}

macro_rules! bounded_read {
    ($name:ident, $wire:literal, $request:ty, $success:ty, $call:path) => {
        pub struct $name;
        impl Operation for $name {
            type Request = $request;
            type Success = provenance_core::protocol::QueryResponse<$success>;
            type Failure = ReadError;
            const NAME: &'static str = $wire;
            const CONTEXT: super::ContextKind = super::ContextKind::Scoped;
            fn needs(_: &Self::Request) -> ExecutionNeeds {
                &[
                    ExecutionNeed::GraphStorage,
                    ExecutionNeed::ProjectionMaintenance,
                ]
            }
            fn failure_status(error: &ReadError) -> u16 {
                error.status()
            }
            fn run(
                context: PreparedContext,
                request: Self::Request,
            ) -> OperationFuture<Self::Success, Self::Failure> {
                Box::pin(async move {
                    let context = context.graph()?;
                    let answer = $call(&context.root, &context.scope, context.policy, request)
                        .await
                        .map_err(ReadError::from)?;
                    Ok(provenance_core::protocol::QueryResponse::new($wire, answer))
                })
            }
        }
    };
}

bounded_read!(
    ReviewHistoryOperation,
    "review-history",
    provenance_core::review::ReviewHistoryQuery,
    provenance_core::review::ReviewHistoryPage,
    crate::review::read_history
);
bounded_read!(
    ReviewEvidenceOperation,
    "review-evidence",
    provenance_core::review::EvidenceQuery,
    provenance_core::review::EvidencePage,
    crate::review::read_evidence
);
bounded_read!(
    ReviewDiscussionsOperation,
    "review-discussions",
    provenance_core::threads::DiscussionQuery,
    provenance_core::threads::DiscussionPage,
    crate::review::read_discussions
);
bounded_read!(
    ReviewDiscussionMessagesOperation,
    "review-discussion-messages",
    provenance_core::threads::DiscussionMessagesQuery,
    provenance_core::threads::DiscussionMessagesPage,
    crate::review::read_discussion_messages
);
