//! Adapters for existing single-record shaping and relationship actions.
use super::ExecutionNeed;
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
        $crate::operations::catalog::shapes::scoped_write_operation!(
            pub $name, $wire, $input, $output, &[], &[ExecutionNeed::GraphStorage],
            scope = scope_id, |$store, _scope, $request| $call
        );
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
