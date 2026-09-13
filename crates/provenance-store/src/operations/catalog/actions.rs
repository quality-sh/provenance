//! Adapters for existing single-record shaping and relationship actions.
use super::{ExecutionNeed, ExecutionNeeds, Operation, OperationFuture, PreparedContext};
use crate::{
    layout::ProvenanceLayout,
    state_store::StateStore,
    write_error::{SourceFailure, WriteError, WriteFailure},
};
use provenance_core::{ScopeId, StableId};
use serde::Deserialize;

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordActionInput {
    pub scope_id: ScopeId,
    pub id: StableId,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimInput {
    pub scope_id: ScopeId,
    pub id: StableId,
    pub actor: String,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnswerQuestionInput {
    pub scope_id: ScopeId,
    pub id: StableId,
    pub answer: String,
    pub resolution_id: Option<StableId>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceActionInput {
    pub scope_id: ScopeId,
    pub id: StableId,
    pub target_id: StableId,
}

macro_rules! native_action {
    ($name:ident, $wire:literal, $input:ty, $output:ty, |$store:ident, $request:ident| $call:expr) => {
        pub struct $name;
        impl Operation for $name {
            type Request = $input;
            type Success = $output;
            type Failure = WriteError;
            const NAME: &'static str = $wire;
            const MUTATES: bool = true;
            const CONTEXT: super::ContextKind = super::ContextKind::Scope;
            fn failure_status(error: &WriteError) -> u16 {
                error.status()
            }
            fn needs(_: &Self::Request) -> ExecutionNeeds {
                &[ExecutionNeed::GraphStorage]
            }
            fn run(
                context: PreparedContext,
                $request: Self::Request,
            ) -> OperationFuture<Self::Success, Self::Failure> {
                Box::pin(async move {
                    let context = context.scope()?;
                    if $request.scope_id != context.scope {
                        return Err(SourceFailure::wrap(
                            WriteFailure::ScopeMismatch,
                            anyhow::anyhow!("request scope does not match selected scope"),
                        )
                        .into());
                    }
                    let $store = StateStore::new(ProvenanceLayout::new(context.root));
                    Ok($call?)
                })
            }
        }
    };
}
pub(super) use native_action;

native_action!(
    ClaimTopic,
    "claim-topic",
    ClaimInput,
    crate::state_store::TopicClaim,
    |store, input| store.claim_topic(&input.scope_id, &input.id, &input.actor)
);
native_action!(
    ReleaseTopic,
    "release-topic",
    RecordActionInput,
    provenance_core::Topic,
    |store, input| store.release_topic(&input.scope_id, &input.id)
);
native_action!(
    CloseTopic,
    "close-topic",
    RecordActionInput,
    provenance_core::Topic,
    |store, input| store.close_topic(&input.scope_id, &input.id)
);
native_action!(
    ClaimQuestion,
    "claim-question",
    ClaimInput,
    provenance_core::Question,
    |store, input| store.claim_question(&input.scope_id, &input.id, &input.actor)
);
native_action!(
    ReleaseQuestion,
    "release-question",
    RecordActionInput,
    provenance_core::Question,
    |store, input| store.release_question(&input.scope_id, &input.id)
);
native_action!(
    AnswerQuestion,
    "answer-question",
    AnswerQuestionInput,
    provenance_core::Question,
    |store, input| store.answer_question(
        &input.scope_id,
        &input.id,
        input.answer,
        input.resolution_id
    )
);
