#![allow(clippy::result_large_err)]
//! Dispatch by operation name: one table entry for each registered operation decodes the
//! JSON call, runs the typed operation on the host, and encodes the result.

use provenance_core::{
    protocol::failure::{
        ErasedFailure as FailureEnvelope, InvalidInputReason, OperationError, OperationFailure,
    },
    SDK_PROTOCOL_VERSION,
};
use provenance_store::operations::{
    catalog::{
        self as operation, frame_failure, ContextKind, Invoke, Operation, OperationFuture,
        RequestedContext,
    },
    family_operations as family,
};
use serde::{de::IntoDeserializer as _, Deserialize};
use serde_json::Value;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DataFreeCall<R> {
    request: R,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RepositoryCall<R, C> {
    context: C,
    request: R,
}

struct Entry<H: Invoke> {
    name: &'static str,
    mutates: bool,
    invoke: fn(Value, H) -> OperationFuture<Value, FailureEnvelope>,
}

fn register<O: Operation, H: Invoke>() -> Entry<H> {
    Entry {
        name: O::NAME,
        mutates: O::MUTATES,
        invoke: invoke_resolved::<O, H>,
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
fn names() -> impl Iterator<Item = (&'static str, bool)> {
    entries::<Unhosted>()
        .into_iter()
        .map(|entry| (entry.name, entry.mutates))
}

/// Lookup uses registry identities without deriving or cloning wire schemas.
pub fn contains(operation: &str) -> bool {
    names().any(|(name, _)| name == operation)
}

pub fn mutates(operation: &str) -> bool {
    names().any(|(name, mutates)| name == operation && mutates)
}

#[cfg(test)]
pub(crate) fn registered_operation_names_for_test() -> Vec<&'static str> {
    names().map(|(name, _)| name).collect()
}

macro_rules! register_family_catalog {
    ($entries:ident, [none], [$($review:tt)*]) => {};
    (
        $entries:ident,
        [projection($list:ident, $list_wire:literal, $page:ident, $page_wire:literal, none)],
        [$($review:tt)*]
    ) => {
        $entries.push(register::<family::$list, H>());
        $entries.push(register::<family::$page, H>());
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
        $entries.push(register::<family::$list, H>());
        $entries.push(register::<family::$page, H>());
        $entries.push(register::<family::$member, H>());
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
        $entries.push(register::<family::$list, H>());
        $entries.push(register::<family::$page, H>());
        $entries.push(register::<family::$member, H>());
    };
}

macro_rules! register_family_entries {
    (
        $entries:ident;
        $($group:ident {
            $($variant:ident {
                record: $record:ty,
                order: $order:tt,
                node: [$($node:tt)*],
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
fn entries<H: Invoke>() -> Vec<Entry<H>> {
    let mut entries = vec![
        register::<operation::CheckStatement, H>(),
        register::<operation::CreateContribution, H>(),
        register::<operation::UpsertContribution, H>(),
        register::<operation::CreateSynthesisPacket, H>(),
        register::<operation::UpsertSynthesisPacket, H>(),
        // Phase 2b replaces these legacy registrations with resource routes.
        register::<operation::AddRequirementRefines, H>(),
        register::<operation::ClearRequirementRefines, H>(),
        register::<operation::AddRequirementDependsOn, H>(),
        register::<operation::ClearRequirementDependsOn, H>(),
        register::<operation::AddRequirementSupersedes, H>(),
        register::<operation::ClearRequirementSupersedes, H>(),
        register::<operation::SetRequirementSpawnedBy, H>(),
        register::<operation::ClearRequirementSpawnedBy, H>(),
        register::<operation::AddRuleRequirement, H>(),
        register::<operation::ClearRuleRequirement, H>(),
        register::<operation::AddRuleResolution, H>(),
        register::<operation::ClearRuleResolution, H>(),
        register::<operation::AddResolutionRequirement, H>(),
        register::<operation::ClearResolutionRequirement, H>(),
        register::<operation::AddResolutionSupersedes, H>(),
        register::<operation::ClearResolutionSupersedes, H>(),
        register::<operation::AddSourceSupersedes, H>(),
        register::<operation::ClearSourceSupersedes, H>(),
        register::<operation::SetQuestionContradicts, H>(),
        register::<operation::ClearQuestionContradicts, H>(),
        register::<operation::ClearSourceReference, H>(),
        register::<operation::ClaimTopic, H>(),
        register::<operation::ReleaseTopic, H>(),
        register::<operation::CloseTopic, H>(),
        register::<operation::ClaimQuestion, H>(),
        register::<operation::ReleaseQuestion, H>(),
        register::<operation::AnswerQuestion, H>(),
        register::<operation::UpdateSource, H>(),
        register::<operation::UpdateResolution, H>(),
        register::<operation::UpdateRequirementNative, H>(),
        register::<operation::UpdateRule, H>(),
        register::<operation::UpdateDomain, H>(),
        register::<operation::UpdateBoundary, H>(),
        register::<operation::UpdateTopic, H>(),
        register::<operation::UpdateQuestion, H>(),
        register::<operation::CreateDomain, H>(),
        register::<operation::CreateBoundary, H>(),
        register::<operation::CreateTopic, H>(),
        register::<operation::CreateQuestion, H>(),
        register::<operation::CreateSource, H>(),
        register::<operation::CreateRequirementNative, H>(),
        register::<operation::CreateRule, H>(),
        register::<operation::CreateResolution, H>(),
        register::<operation::AddSourceReference, H>(),
        register::<operation::Plan, H>(),
        register::<operation::Apply, H>(),
        register::<operation::BeginVerification, H>(),
        register::<operation::CompleteVerification, H>(),
        register::<operation::Info, H>(),
        register::<operation::Get, H>(),
        register::<operation::ReadDocument, H>(),
        register::<operation::Search, H>(),
        register::<operation::Neighbors, H>(),
        register::<operation::Trace, H>(),
        register::<operation::Impact, H>(),
        register::<operation::ResolveSymbol, H>(),
        register::<operation::Evidence, H>(),
        register::<operation::Stale, H>(),
        register::<operation::VerificationRuns, H>(),
        register::<operation::VerificationBindings, H>(),
        register::<operation::ListThreads, H>(),
        register::<operation::PostThreadMessage, H>(),
        register::<operation::CreateProposal, H>(),
        register::<operation::CreateAssertion, H>(),
        register::<operation::CreateDisposition, H>(),
        register::<family::PageProposalAssertions, H>(),
        register::<family::PageProposalDispositions, H>(),
        register::<family::PageVerificationRuns, H>(),
        register::<family::GetVerificationRun, H>(),
        register::<family::GetProposalAssertion, H>(),
        register::<family::GetProposalDisposition, H>(),
        register::<family::GetReviewedResource, H>(),
        register::<family::ListVerificationRuns, H>(),
        register::<operation::CreateRequirementResource, H>(),
        register::<operation::UpdateRequirementResource, H>(),
        register::<operation::SubmitRecordReview, H>(),
        register::<operation::DecideRecordReview, H>(),
        register::<operation::WithdrawRecordReview, H>(),
        register::<operation::ReviewHistory, H>(),
        register::<operation::ReviewHistoryEntry, H>(),
        register::<operation::ReviewEvidence, H>(),
        register::<operation::ReviewDiscussions, H>(),
        register::<operation::ReviewDiscussion, H>(),
        register::<operation::ReviewDiscussionMessages, H>(),
        register::<operation::ReviewDiscussionMessage, H>(),
        register::<operation::ListDiscussions, H>(),
        register::<operation::GetDiscussionConversation, H>(),
        register::<operation::WriteDiscussion, H>(),
        register::<operation::WriteTargetDiscussion, H>(),
    ];
    provenance_store::record_family_catalog!(register_family_entries, entries);
    entries
}

pub async fn invoke_with<H: Invoke>(
    operation: &str,
    version: u32,
    call: Value,
    host: H,
) -> Result<Value, FailureEnvelope> {
    if version != SDK_PROTOCOL_VERSION {
        return Err(FailureEnvelope::new(
            None,
            OperationFailure::ProtocolMismatch {
                requested: version,
                supported: SDK_PROTOCOL_VERSION,
            },
        ));
    }
    let entry = entries::<H>()
        .into_iter()
        .find(|entry| entry.name == operation)
        .ok_or_else(|| FailureEnvelope::new(None, OperationFailure::UnknownOperation))?;
    (entry.invoke)(call, host).await
}

fn invoke_resolved<O: Operation, H: Invoke>(
    call: Value,
    host: H,
) -> OperationFuture<Value, FailureEnvelope> {
    Box::pin(async move {
        let (selected, request) = match O::CONTEXT {
            ContextKind::Repository => {
                let call: RepositoryCall<
                    O::Request,
                    provenance_core::protocol::repository::RepositoryTarget,
                > = decode::<O, _>(call)?;
                (
                    Some(RequestedContext::Repository(call.context)),
                    call.request,
                )
            }
            ContextKind::Scope => {
                let call: RepositoryCall<
                    O::Request,
                    provenance_core::protocol::repository::RepositoryScope,
                > = decode::<O, _>(call)?;
                (Some(RequestedContext::Scope(call.context)), call.request)
            }
            ContextKind::Scoped => {
                let call: RepositoryCall<
                    O::Request,
                    provenance_core::protocol::repository::RepositoryContext,
                > = decode::<O, _>(call)?;
                (Some(RequestedContext::Scoped(call.context)), call.request)
            }
            ContextKind::DataFree => {
                let call: DataFreeCall<O::Request> = decode::<O, _>(call)?;
                (None, call.request)
            }
        };
        let success = host
            .invoke::<O>(selected, request)
            .await
            .map_err(frame_failure::<O>)?;
        serde_json::to_value(success)
            .map_err(|_| FailureEnvelope::new(Some(O::NAME), OperationFailure::Internal))
    })
}

#[provenance_macros::rule("rule_submit_refusal_names_missing_lists")]
fn decode<O: Operation, T: serde::de::DeserializeOwned>(call: Value) -> Result<T, FailureEnvelope> {
    let schema = crate::schema::schema::<O::Request>(schemars::generate::Contract::Deserialize);
    let missing = schema["required"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter(|field| {
            schema["properties"][*field]["type"] == "array" && call["request"].get(*field).is_none()
        })
        .collect::<Vec<_>>();
    if let Some(first) = missing.first() {
        let guidance = missing
            .iter()
            .map(|field| {
                format!(
                    "{field}: supply --{} '[]' or JSON \"{field}\": []",
                    crate::schema::json_flag(field)
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        return Err(FailureEnvelope::declared(
            O::NAME,
            serde_json::json!({
                "kind":"invalid_input", "field":format!("request.{first}"), "reason":"required",
                "missing_fields":missing, "message":format!("Required list fields are missing: {guidance}")
            }),
            400,
        ));
    }
    serde_path_to_error::deserialize(call.into_deserializer()).map_err(|error| {
        let path = error.path().to_string();
        let parent = (!path.is_empty() && path != ".").then_some(path);
        let message = error.inner().to_string();
        // A missing field is named under its parent, as the list check above does.
        let failure = match missing_field(&message) {
            Some(name) => OperationFailure::InvalidInput {
                field: Some(
                    parent.map_or_else(|| name.to_owned(), |parent| format!("{parent}.{name}")),
                ),
                reason: InvalidInputReason::Required,
            },
            None => OperationFailure::InvalidInput {
                field: parent,
                reason: InvalidInputReason::InvalidValue,
            },
        };
        FailureEnvelope::new(Some(O::NAME), failure)
    })
}

/// The field that a serde "missing field" error names. The
/// `missing_field_text_is_pinned` test fails if serde changes this text.
fn missing_field(message: &str) -> Option<&str> {
    message
        .strip_prefix("missing field `")
        .and_then(|rest| rest.strip_suffix('`'))
}

#[cfg(test)]
mod tests {
    use serde::de::IntoDeserializer as _;

    #[derive(Debug, serde::Deserialize)]
    #[allow(dead_code)]
    struct Probe {
        actor: String,
    }

    #[test]
    fn missing_field_text_is_pinned() {
        let call = serde_json::json!({});
        let error =
            serde_path_to_error::deserialize::<_, Probe>(call.into_deserializer()).unwrap_err();
        let message = error.inner().to_string();
        assert_eq!(message, "missing field `actor`");
        assert_eq!(super::missing_field(&message), Some("actor"));
    }
}
