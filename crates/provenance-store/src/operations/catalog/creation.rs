//! Existing native creation inputs retain their scope and placement fields.
use super::{ExecutionNeed, ExecutionNeeds, Operation, OperationFuture, PreparedContext};
use crate::write_error::{SourceFailure, WriteFailure};
use crate::{
    layout::ProvenanceLayout,
    state_store::{
        AddSourceReferenceInput, CreateRequirementInput, CreateResolutionInput, CreateRuleInput,
        CreateSourceInput, StateStore,
    },
    write_error::WriteError,
};

macro_rules! creation {
    ($name:ident, $wire:literal, $input:ty, $output:ty, $method:ident, [$($need:ident),*]) => {
        pub struct $name;
        impl Operation for $name {
            type Request = $input;
            type Success = $output;
            type Failure = WriteError;
            const NAME: &'static str = $wire;
            const MUTATES: bool = true;
            const CONTEXT: super::ContextKind = super::ContextKind::Scope;
            const FAILURE_STATUSES: &'static [u16] = &[409];
            fn failure_status(error: &WriteError) -> u16 { error.status() }
            fn needs(_: &Self::Request) -> ExecutionNeeds { &[$(ExecutionNeed::$need),*] }
            fn run(context: PreparedContext, request: Self::Request) -> OperationFuture<Self::Success, Self::Failure> {
                Box::pin(async move {
                    let context = context.scope()?;
                    if request.scope_id != context.scope {
                        return Err(SourceFailure::wrap(WriteFailure::ScopeMismatch,
                            anyhow::anyhow!("request scope does not match selected scope")).into());
                    }
                    Ok(StateStore::new(ProvenanceLayout::new(context.root)).$method(request)?)
                })
            }
        }
    };
}
creation!(
    CreateSource,
    "create-source",
    CreateSourceInput,
    provenance_core::Source,
    create_source,
    [GraphStorage]
);
creation!(
    CreateRequirement,
    "create-requirement",
    CreateRequirementInput,
    provenance_core::Requirement,
    create_requirement,
    [GraphStorage, Dictionary]
);
creation!(
    CreateResolution,
    "create-resolution",
    CreateResolutionInput,
    provenance_core::Resolution,
    create_resolution,
    [GraphStorage]
);
creation!(
    CreateRule,
    "create-rule",
    CreateRuleInput,
    provenance_core::Rule,
    create_rule,
    [GraphStorage, Dictionary]
);
creation!(
    AddSourceReference,
    "add-source-reference",
    AddSourceReferenceInput,
    provenance_core::Requirement,
    add_source_reference,
    [GraphStorage]
);

creation!(
    PostThreadMessage,
    "post-thread-message",
    crate::state_store::PostMessageInput,
    crate::state_store::PostMessageResult,
    post_thread_message,
    [GraphStorage]
);

pub(super) use creation;
