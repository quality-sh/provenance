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

#[allow(clippy::too_many_lines)]
pub(super) fn entries() -> Vec<Entry> {
    vec![
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
        register::<super::ResolveRecord>(),
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
        register::<super::resource_pages::PageSourcesV2>(),
        register::<super::resource_pages::PageRequirementsV2>(),
        register::<super::resource_pages::PageResolutionsV2>(),
        register::<super::resource_pages::PageRulesV2>(),
        register::<super::resource_pages::PageDomainsV2>(),
        register::<super::resource_pages::PageBoundariesV2>(),
        register::<super::resource_pages::PageTopicsV2>(),
        register::<super::resource_pages::PageQuestionsV2>(),
        register::<super::resource_pages::PageContributionsV2>(),
        register::<super::resource_pages::PageSynthesisPacketsV2>(),
        register::<super::resource_pages::PageProposalsV2>(),
        register::<super::resource_pages::PageVerificationBindingsV2>(),
        register::<super::resource_pages::PageDiscussionContainersV2>(),
        register::<super::resource_pages::PageMessagesV2>(),
        register::<super::resource_pages::PageAssertionsV2>(),
        register::<super::resource_pages::PageDispositionsV2>(),
        register::<super::resource_pages::PageProposalAssertionsV2>(),
        register::<super::resource_pages::PageProposalDispositionsV2>(),
        register::<super::verification_resources::PageVerificationRunsV2>(),
        register::<super::resource_members::GetSourceV2>(),
        register::<super::resource_members::GetResolutionV2>(),
        register::<super::resource_members::GetRuleV2>(),
        register::<super::resource_members::GetDomainV2>(),
        register::<super::resource_members::GetBoundaryV2>(),
        register::<super::resource_members::GetTopicV2>(),
        register::<super::resource_members::GetQuestionV2>(),
        register::<super::resource_members::GetContributionV2>(),
        register::<super::resource_members::GetSynthesisPacketV2>(),
        register::<super::resource_members::GetProposalV2>(),
        register::<super::verification_resources::GetVerificationRunV2>(),
        register::<super::resource_members::GetVerificationBindingV2>(),
        register::<super::resource_members::GetDiscussionContainerV2>(),
        register::<super::resource_members::GetMessageV2>(),
        register::<super::resource_members::GetAssertionV2>(),
        register::<super::resource_members::GetDispositionV2>(),
        register::<super::resource_members::GetProposalAssertionV2>(),
        register::<super::resource_members::GetProposalDispositionV2>(),
        register::<super::resource_lists::ListSourcesV2>(),
        register::<super::resource_lists::ListRequirementsV2>(),
        register::<super::resource_lists::ListResolutionsV2>(),
        register::<super::resource_lists::ListRulesV2>(),
        register::<super::resource_lists::ListDomainsV2>(),
        register::<super::resource_lists::ListBoundariesV2>(),
        register::<super::resource_lists::ListTopicsV2>(),
        register::<super::resource_lists::ListQuestionsV2>(),
        register::<super::resource_lists::ListContributionsV2>(),
        register::<super::resource_lists::ListSynthesisPacketsV2>(),
        register::<super::resource_lists::ListProposalsV2>(),
        register::<super::resource_lists::ListVerificationRunsV2>(),
        register::<super::resource_lists::ListVerificationBindingsV2>(),
        register::<super::resource_lists::ListDiscussionContainersV2>(),
        register::<super::resource_lists::ListMessagesV2>(),
        register::<super::resource_lists::ListAssertionsV2>(),
        register::<super::resource_lists::ListDispositionsV2>(),
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
        register::<super::WriteDiscussionV2>(),
    ]
}
