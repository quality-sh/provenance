//! Existing native creation inputs retain their scope and placement fields.
use super::ExecutionNeed;
use crate::state_store::{
    AddSourceReferenceInput, CreateRequirementInput, CreateResolutionInput, CreateRuleInput,
    CreateSourceInput,
};

macro_rules! creation {
    ($name:ident, $wire:literal, $input:ty, $output:ty, $method:ident, [$($need:ident),*]) => {
        $crate::operations::catalog::shapes::scoped_write_operation!(
            pub $name, $wire, $input, $output, &[409], &[$(ExecutionNeed::$need),*],
            scope = scope_id, |store, _scope, request| store.$method(request)
        );
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
