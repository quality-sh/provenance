use super::{ExecutionNeeds, PreparedContext};
use provenance_core::protocol::failure::ErasedFailure as FailureEnvelope;
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

pub(super) struct Entry {
    pub name: &'static str,
    pub mutates: bool,
    pub invoke: fn(
        Value,
        std::sync::Arc<dyn super::ContextResolver>,
    ) -> OperationFuture<Value, FailureEnvelope>,
}

fn register<O: Operation>() -> Entry {
    Entry {
        name: O::NAME,
        mutates: O::MUTATES,
        invoke: super::invoke::invoke_resolved::<O>,
    }
}

macro_rules! register_family_catalog {
    ($entries:ident, none) => {};
    (
        $entries:ident,
        projection($list:ident, $list_wire:literal, $page:ident, $page_wire:literal, none)
    ) => {
        $entries.push(register::<super::resource_lists::$list>());
        $entries.push(register::<super::resource_pages::$page>());
    };
    (
        $entries:ident,
        $kind:ident(
            $list:ident,
            $list_wire:literal,
            $page:ident,
            $page_wire:literal,
            $member:ident,
            $member_wire:literal
        )
    ) => {
        $entries.push(register::<super::resource_lists::$list>());
        $entries.push(register::<super::resource_pages::$page>());
        $entries.push(register::<super::resource_members::$member>());
    };
    (
        $entries:ident,
        verification(
            $list:ident,
            $list_wire:literal,
            $page:ident,
            $member:ident,
            $member_wire:literal
        )
    ) => {
        $entries.push(register::<super::resource_lists::$list>());
        $entries.push(register::<super::resource_pages::$page>());
        $entries.push(register::<super::resource_members::$member>());
    };
}

macro_rules! register_family_entries {
    (
        $entries:ident;
        $(
            $group:ident {
                $(
                    $variant:ident {
                        record: $record:ty,
                        field: $field:ident,
                        path: $path:ident,
                        node: [$($node:tt)*],
                        reader: $reader:ident,
                        closed: [$($closed:tt)*],
                        strategy: $strategy:ident,
                        id: $id:ident,
                        loader: [$($loader:tt)*],
                        catalog: [$($catalog:tt)*]
                    };
                )*
            }
        )*
    ) => {
        $($(register_family_catalog!($entries, $($catalog)*);)*)*
    };
}

#[allow(clippy::too_many_lines)]
pub(super) fn entries() -> Vec<Entry> {
    let mut entries = vec![
        register::<super::CheckStatement>(),
        register::<super::CreateContribution>(),
        register::<super::UpsertContribution>(),
        register::<super::CreateSynthesisPacket>(),
        register::<super::UpsertSynthesisPacket>(),
        // Phase 2b replaces these legacy registrations with resource routes.
        register::<super::SetRequirementRefines>(),
        register::<super::ClearRequirementRefines>(),
        register::<super::AddRequirementDependsOn>(),
        register::<super::ClearRequirementDependsOn>(),
        register::<super::AddRequirementSupersedes>(),
        register::<super::ClearRequirementSupersedes>(),
        register::<super::SetRequirementSpawnedBy>(),
        register::<super::ClearRequirementSpawnedBy>(),
        register::<super::AddRuleRequirement>(),
        register::<super::ClearRuleRequirement>(),
        register::<super::AddRuleResolution>(),
        register::<super::ClearRuleResolution>(),
        register::<super::AddResolutionRequirement>(),
        register::<super::ClearResolutionRequirement>(),
        register::<super::AddResolutionSupersedes>(),
        register::<super::ClearResolutionSupersedes>(),
        register::<super::AddSourceSupersedes>(),
        register::<super::ClearSourceSupersedes>(),
        register::<super::SetQuestionContradicts>(),
        register::<super::ClearQuestionContradicts>(),
        register::<super::ClearSourceReference>(),
        register::<super::ClaimTopic>(),
        register::<super::ReleaseTopic>(),
        register::<super::CloseTopic>(),
        register::<super::ClaimQuestion>(),
        register::<super::ReleaseQuestion>(),
        register::<super::AnswerQuestion>(),
        register::<super::UpdateSource>(),
        register::<super::UpdateResolution>(),
        register::<super::UpdateRequirement>(),
        register::<super::UpdateRule>(),
        register::<super::UpdateDomain>(),
        register::<super::UpdateBoundary>(),
        register::<super::UpdateTopic>(),
        register::<super::UpdateQuestion>(),
        register::<super::CreateDomain>(),
        register::<super::CreateBoundary>(),
        register::<super::CreateTopic>(),
        register::<super::CreateQuestion>(),
        register::<super::CreateSource>(),
        register::<super::CreateRequirement>(),
        register::<super::CreateRule>(),
        register::<super::CreateResolution>(),
        register::<super::AddSourceReference>(),
        register::<super::Plan>(),
        register::<super::Apply>(),
        register::<super::BeginVerification>(),
        register::<super::CompleteVerification>(),
        register::<super::Info>(),
        register::<super::Get>(),
        register::<super::ReadDocument>(),
        register::<super::Search>(),
        register::<super::Neighbors>(),
        register::<super::Trace>(),
        register::<super::Impact>(),
        register::<super::ResolveSymbol>(),
        register::<super::Evidence>(),
        register::<super::Stale>(),
        register::<super::VerificationRuns>(),
        register::<super::VerificationBindings>(),
        register::<super::ListThreads>(),
        register::<super::ListMessages>(),
        register::<super::PostThreadMessage>(),
        register::<super::ListProposals>(),
        register::<super::ListDispositions>(),
        register::<super::ListAssertions>(),
        register::<super::CreateProposal>(),
        register::<super::CreateAssertion>(),
        register::<super::CreateDisposition>(),
        register::<super::resource_pages::PageProposalAssertionsV2>(),
        register::<super::resource_pages::PageProposalDispositionsV2>(),
        register::<super::verification_resources::PageVerificationRunsV2>(),
        register::<super::verification_resources::GetVerificationRunV2>(),
        register::<super::resource_members::GetProposalAssertionV2>(),
        register::<super::resource_members::GetProposalDispositionV2>(),
        register::<super::resource_lists::ListVerificationRunsV2>(),
        register::<super::GetRequirementV2>(),
        register::<super::CreateRequirementV2>(),
        register::<super::UpdateRequirementV2>(),
        register::<super::SubmitRequirementReviewV2>(),
        register::<super::DecideRequirementReviewV2>(),
        register::<super::WithdrawRequirementReviewV2>(),
        register::<super::ReviewHistoryV2>(),
        register::<super::ReviewHistoryEntryV2>(),
        register::<super::ReviewEvidenceV2>(),
        register::<super::ReviewDiscussionsV2>(),
        register::<super::ReviewDiscussionV2>(),
        register::<super::ReviewDiscussionMessagesV2>(),
        register::<super::ReviewDiscussionMessageV2>(),
        register::<super::ListDiscussionsV2>(),
        register::<super::GetDiscussionConversationV2>(),
        register::<super::WriteDiscussionV2>(),
        register::<super::WriteTargetDiscussionV2>(),
    ];
    crate::cache::record_families!(register_family_entries, entries);
    entries
}
