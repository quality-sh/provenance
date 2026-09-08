//! Typed operation dispatch shared by native and network callers.

mod context;
mod entry;
mod invoke;
#[cfg(feature = "schema")]
mod schema;
mod statement;

pub use context::{ExecutionNeed, ExecutionNeeds, PreparedContext};
pub use entry::{Operation, OperationFuture, WireSchema};
pub use invoke::{invoke, invoke_typed};
#[cfg(feature = "schema")]
pub use schema::{bind_response_identity, definitions, Definition};
pub use statement::CheckStatement;

#[cfg(test)]
mod tests;
