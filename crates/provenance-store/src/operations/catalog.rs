//! Typed operation dispatch shared by native and network callers.

mod authoring;
mod context;
mod creation;
mod discussions;
mod entry;
mod evidence;
mod failures;
mod ideation;
mod invoke;
mod records;
#[cfg(feature = "schema")]
mod schema;
mod scoped_list;
mod statement;

pub use authoring::{Apply, BeginVerification, CompleteVerification, Plan};
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
pub use invoke::{invoke, invoke_typed, invoke_with};
#[cfg(feature = "schema")]
pub use schema::{bind_response_identity, definitions, Definition};
pub use statement::CheckStatement;

#[cfg(test)]
mod tests;

pub use records::{Get, Info, Neighbors, Search, Trace};

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
