//! Typed operation dispatch shared by native and network callers.

mod actions;
mod authoring;
#[cfg(feature = "schema")]
mod binding;
mod relationships;
pub use actions::*;
pub use relationships::*;
mod context;
mod creation;
mod discussion_discovery;
mod discussions;
mod drafts;
pub use drafts::*;
mod entry;
mod evidence;
mod failures;
mod ideation;
mod invoke;
mod records;
mod resource_lists;
mod resource_members;
mod resource_pages;
#[cfg(feature = "schema")]
mod routes;
#[cfg(feature = "schema")]
mod schema;
#[cfg(feature = "schema")]
mod schema_page;
#[cfg(feature = "schema")]
mod schema_values;
mod scoped_list;
mod statement;
mod target_discussion_writes;
mod updates;
mod v2_discussion_reads;
mod v2_review;
mod v2_review_reads;
mod verification_resources;
pub use discussion_discovery::*;
pub use updates::*;
pub use v2_discussion_reads::*;
pub use v2_review::*;
pub use v2_review_reads::*;

pub use authoring::{Apply, BeginVerification, CompleteVerification, Plan};
#[cfg(feature = "schema")]
pub use binding::{
    ArgumentAlias, CliBinding, CliDefault, CliDefaultValue, Controls, EtagBinding, HandlerBinding,
    HeaderBinding, NullClearBinding, ParentBinding, PathBinding, QueryRequestBinding, QueryRoute,
    Registration, RequestAdapter, RequestAdapterError, RequestBinding, ResponseAdapter,
    ResponseBinding, SelectorBinding, TargetAction, TargetBinding,
};
pub use context::{
    ContextKind, ContextResolver, ExecutionNeed, ExecutionNeeds, PreparedContext, PreparedRead,
    PreparedRepository, PreparedScope, RequestedContext,
};
pub use creation::{
    AddSourceReference, CreateRequirement, CreateResolution, CreateRule, CreateSource,
    PostThreadMessage,
};
pub use discussions::{ListMessages, ListThreads};
pub use entry::{Operation, OperationFuture, WireSchema};
pub use ideation::{
    CreateAssertion, CreateDisposition, CreateProposal, ListAssertions, ListDispositions,
    ListProposals,
};
pub use invoke::{
    invoke, invoke_authorized_native_typed, invoke_authorized_typed, invoke_typed, invoke_with,
};
#[cfg(feature = "schema")]
pub use schema::{
    definitions, operation_request_schema, operation_success_schema, parse_parameter_value,
    parse_schema_value, parse_schema_value_in, serialize_parameter_value, target_definition,
    target_definitions, Definition, HttpMethod, Parameter, ParseValueError, QueryVariant,
    ResponseKind,
};
pub use statement::CheckStatement;
pub use target_discussion_writes::WriteTargetDiscussionV2;

#[cfg(test)]
mod tests;

pub use records::{Get, Info, Neighbors, ReadDocument, ResolveRecord, Search, Trace};

/// Lookup uses registry identities without deriving or cloning wire schemas.
pub fn contains(operation: &str) -> bool {
    entry::entries().iter().any(|entry| entry.name == operation)
}

pub use evidence::{
    Evidence, Impact, ResolveSymbol, Stale, VerificationBindings, VerificationRuns,
};

pub fn mutates(operation: &str) -> bool {
    entry::entries()
        .iter()
        .any(|entry| entry.name == operation && entry.mutates)
}
