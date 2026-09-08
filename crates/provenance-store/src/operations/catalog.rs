//! Typed operation dispatch shared by native and network callers.

mod context;
mod entry;
mod failures;
mod invoke;
mod records;
#[cfg(feature = "schema")]
mod schema;
mod statement;

pub use context::{
    ContextKind, ContextResolver, ExecutionNeed, ExecutionNeeds, PreparedContext, PreparedRead,
    PreparedRepository, RequestedContext,
};
pub use entry::{Operation, OperationFuture, WireSchema};
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
