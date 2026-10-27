//! Contract code: this module later leaves the store as one unit with entry, invoke, routes
//! and schema. Add no store state here.
use super::{ExecutionNeeds, PreparedContext, RequestedContext};
use provenance_core::protocol::failure::{
    ErasedFailure as FailureEnvelope, OperationError, OperationFailure,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use std::{future::Future, pin::Pin};

pub type OperationFuture<T, E> = Pin<Box<dyn Future<Output = Result<T, E>> + Send>>;

#[cfg(feature = "schema")]
pub trait WireSchema: schemars::JsonSchema {}
#[cfg(feature = "schema")]
impl<T: schemars::JsonSchema> WireSchema for T {}
#[cfg(not(feature = "schema"))]
pub trait WireSchema {}
#[cfg(not(feature = "schema"))]
impl<T> WireSchema for T {}

/// The associated types bind wire definitions to the invoked handler.
pub trait Operation: Send + Sync + 'static {
    type Request: DeserializeOwned + WireSchema + Send + 'static;
    type Success: Serialize + WireSchema + Send + 'static;
    type Failure: std::error::Error + Serialize + WireSchema + Send + 'static;
    const NAME: &'static str;
    const MUTATES: bool = false;
    const CONTEXT: super::ContextKind = super::ContextKind::DataFree;
    const FAILURE_STATUSES: &'static [u16] = &[];
    fn failure_status(_: &Self::Failure) -> u16 {
        500
    }
    fn validate_external(
        _: &Self::Request,
    ) -> Result<(), provenance_core::protocol::failure::OperationFailure> {
        Ok(())
    }
    fn needs(request: &Self::Request) -> ExecutionNeeds;
    fn run(
        context: PreparedContext,
        request: Self::Request,
    ) -> OperationFuture<Self::Success, Self::Failure>;
}

#[derive(Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub(super) struct DataFreeCall<R> {
    pub request: R,
}

#[derive(Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub(super) struct RepositoryCall<R, C> {
    pub context: C,
    pub request: R,
}

/// A host that runs one typed operation; the catalog entry decodes and encodes JSON.
pub trait Invoke: Clone + Send + Sync + 'static {
    fn invoke<O: Operation>(
        self,
        selected: Option<RequestedContext>,
        request: O::Request,
    ) -> OperationFuture<O::Success, OperationError<O::Failure>>;
}

pub(super) struct Entry<H: Invoke> {
    pub name: &'static str,
    pub mutates: bool,
    pub invoke: fn(Value, H) -> OperationFuture<Value, FailureEnvelope>,
}

fn register<O: Operation, H: Invoke>() -> Entry<H> {
    Entry {
        name: O::NAME,
        mutates: O::MUTATES,
        invoke: super::invoke::invoke_resolved::<O, H>,
    }
}

/// Name-only lookups instantiate the table with a host that runs nothing.
#[derive(Clone)]
struct Unhosted;

impl Invoke for Unhosted {
    fn invoke<O: Operation>(
        self,
        _: Option<RequestedContext>,
        _: O::Request,
    ) -> OperationFuture<O::Success, OperationError<O::Failure>> {
        Box::pin(async { Err(OperationError::Common(OperationFailure::UnavailableNeeds)) })
    }
}

/// Every registered operation name with its mutation flag.
pub(super) fn names() -> impl Iterator<Item = (&'static str, bool)> {
    entries::<Unhosted>()
        .into_iter()
        .map(|entry| (entry.name, entry.mutates))
}

macro_rules! register_family_catalog {
    ($entries:ident, [none], [$($review:tt)*]) => {};
    (
        $entries:ident,
        [projection($list:ident, $list_wire:literal, $page:ident, $page_wire:literal, none)],
        [$($review:tt)*]
    ) => {
        $entries.push(register::<super::resource_lists::$list, H>());
        $entries.push(register::<super::resource_pages::$page, H>());
    };
    (
        $entries:ident,
        [$kind:ident(
            $list:ident,
            $list_wire:literal,
            $page:ident,
            $page_wire:literal,
            $member:ident,
            $member_wire:literal
        )],
        [$review:ident]
    ) => {
        $entries.push(register::<super::resource_lists::$list, H>());
        $entries.push(register::<super::resource_pages::$page, H>());
        $entries.push(register::<super::resource_members::$member, H>());
    };
    (
        $entries:ident,
        [$kind:ident(
            $list:ident,
            $list_wire:literal,
            $page:ident,
            $page_wire:literal,
            $member:ident,
            $member_wire:literal
        )],
        []
    ) => {
        $entries.push(register::<super::resource_lists::$list, H>());
        $entries.push(register::<super::resource_pages::$page, H>());
        $entries.push(register::<super::resource_members::$member, H>());
    };
}

macro_rules! register_family_entries {
    (
        $entries:ident;
        $($group:ident {
            $($variant:ident {
                record: $record:ty,
                field: $field:ident,
                path: $path:ident,
                meta: $meta:tt,
                node: [$($node:tt)*],
                reader: {
                    open: $reader:ident,
                    closed: [$($closed:tt)*],
                    strategy: $strategy:ident
                },
                id: $id:ident,
                loader: [$($loader:tt)*],
                graph: [$($graph:tt)*],
                import: [$($import:tt)*],
                catalog: [$($catalog:tt)*],
                route: [$($route:tt)*]
                $(, review: $review:ident)?
            };)*
        })*
    ) => {
        $($(register_family_catalog!(
            $entries, [$($catalog)*], [$($review)?]
        );)*)*
    };
}

#[allow(clippy::too_many_lines)]
pub(super) fn entries<H: Invoke>() -> Vec<Entry<H>> {
    let mut entries = vec![
        register::<super::CheckStatement, H>(),
        register::<super::CreateContribution, H>(),
        register::<super::UpsertContribution, H>(),
        register::<super::CreateSynthesisPacket, H>(),
        register::<super::UpsertSynthesisPacket, H>(),
        // Phase 2b replaces these legacy registrations with resource routes.
        register::<super::SetRequirementRefines, H>(),
        register::<super::ClearRequirementRefines, H>(),
        register::<super::AddRequirementDependsOn, H>(),
        register::<super::ClearRequirementDependsOn, H>(),
        register::<super::AddRequirementSupersedes, H>(),
        register::<super::ClearRequirementSupersedes, H>(),
        register::<super::SetRequirementSpawnedBy, H>(),
        register::<super::ClearRequirementSpawnedBy, H>(),
        register::<super::AddRuleRequirement, H>(),
        register::<super::ClearRuleRequirement, H>(),
        register::<super::AddRuleResolution, H>(),
        register::<super::ClearRuleResolution, H>(),
        register::<super::AddResolutionRequirement, H>(),
        register::<super::ClearResolutionRequirement, H>(),
        register::<super::AddResolutionSupersedes, H>(),
        register::<super::ClearResolutionSupersedes, H>(),
        register::<super::AddSourceSupersedes, H>(),
        register::<super::ClearSourceSupersedes, H>(),
        register::<super::SetQuestionContradicts, H>(),
        register::<super::ClearQuestionContradicts, H>(),
        register::<super::ClearSourceReference, H>(),
        register::<super::ClaimTopic, H>(),
        register::<super::ReleaseTopic, H>(),
        register::<super::CloseTopic, H>(),
        register::<super::ClaimQuestion, H>(),
        register::<super::ReleaseQuestion, H>(),
        register::<super::AnswerQuestion, H>(),
        register::<super::UpdateSource, H>(),
        register::<super::UpdateResolution, H>(),
        register::<super::UpdateRequirementNative, H>(),
        register::<super::UpdateRule, H>(),
        register::<super::UpdateDomain, H>(),
        register::<super::UpdateBoundary, H>(),
        register::<super::UpdateTopic, H>(),
        register::<super::UpdateQuestion, H>(),
        register::<super::CreateDomain, H>(),
        register::<super::CreateBoundary, H>(),
        register::<super::CreateTopic, H>(),
        register::<super::CreateQuestion, H>(),
        register::<super::CreateSource, H>(),
        register::<super::CreateRequirementNative, H>(),
        register::<super::CreateRule, H>(),
        register::<super::CreateResolution, H>(),
        register::<super::AddSourceReference, H>(),
        register::<super::Plan, H>(),
        register::<super::Apply, H>(),
        register::<super::BeginVerification, H>(),
        register::<super::CompleteVerification, H>(),
        register::<super::Info, H>(),
        register::<super::Get, H>(),
        register::<super::ReadDocument, H>(),
        register::<super::Search, H>(),
        register::<super::Neighbors, H>(),
        register::<super::Trace, H>(),
        register::<super::Impact, H>(),
        register::<super::ResolveSymbol, H>(),
        register::<super::Evidence, H>(),
        register::<super::Stale, H>(),
        register::<super::VerificationRuns, H>(),
        register::<super::VerificationBindings, H>(),
        register::<super::ListThreads, H>(),
        register::<super::PostThreadMessage, H>(),
        register::<super::CreateProposal, H>(),
        register::<super::CreateAssertion, H>(),
        register::<super::CreateDisposition, H>(),
        register::<super::resource_pages::PageProposalAssertions, H>(),
        register::<super::resource_pages::PageProposalDispositions, H>(),
        register::<super::verification_resources::PageVerificationRuns, H>(),
        register::<super::verification_resources::GetVerificationRun, H>(),
        register::<super::resource_members::GetProposalAssertion, H>(),
        register::<super::resource_members::GetProposalDisposition, H>(),
        register::<super::resource_members::GetReviewedResource, H>(),
        register::<super::resource_lists::ListVerificationRuns, H>(),
        register::<super::CreateRequirementResource, H>(),
        register::<super::UpdateRequirementResource, H>(),
        register::<super::SubmitRecordReview, H>(),
        register::<super::DecideRecordReview, H>(),
        register::<super::WithdrawRecordReview, H>(),
        register::<super::ReviewHistory, H>(),
        register::<super::ReviewHistoryEntry, H>(),
        register::<super::ReviewEvidence, H>(),
        register::<super::ReviewDiscussions, H>(),
        register::<super::ReviewDiscussion, H>(),
        register::<super::ReviewDiscussionMessages, H>(),
        register::<super::ReviewDiscussionMessage, H>(),
        register::<super::ListDiscussions, H>(),
        register::<super::GetDiscussionConversation, H>(),
        register::<super::WriteDiscussion, H>(),
        register::<super::WriteTargetDiscussion, H>(),
    ];
    crate::cache::family_table::record_family_rows!(register_family_entries, entries);
    entries
}
