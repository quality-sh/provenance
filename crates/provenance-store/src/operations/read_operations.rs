//! The read operations of one repository, as a port.
//!
//! The contract router sends each of these operations to the port of the
//! repository it serves, when the repository has one, and does not run the
//! operation itself. So an embedder can run the reads beside the read
//! database, in another process, and serve the contract routes from a
//! repository that has only its canonical state.
//!
//! A request is the call of one operation as the contract dispatch takes
//! it: `{"context": {...}, "request": {...}}`. An answer is the encoded
//! success, or the failure that the operation declares, with its status.
//!
//! [`InProcessReadOperations`] runs each operation on one opened
//! repository, with the preparation of [`LocalRepository`]. It gives the
//! answers that the router gives when it runs the operation itself.
//!
//! [`LocalRepository`]: super::LocalRepository

use provenance_core::protocol::failure::ErasedFailure;
use serde_json::Value;
use std::fmt::Debug;
use std::future::Future;
use std::pin::Pin;

mod in_process;
pub use in_process::{checked_context, InProcessReadOperations};

/// The answer of one read operation.
pub type ReadAnswer = Pin<Box<dyn Future<Output = Result<Value, ErasedFailure>> + Send>>;

/// Runs the read operations of one repository.
pub trait ReadOperations: Send + Sync + Debug {
    /// Runs `operation`, one of [`READ_OPERATIONS`], on the call `request`.
    fn run(&self, operation: &'static str, request: Value) -> ReadAnswer;
}

/// The wire names of the operations that a [`ReadOperations`] port runs.
///
/// They are the page of each record family that the review page lists, one
/// reviewed record, the document of a Requirement, and the Discussions and
/// messages of a record.
pub const READ_OPERATIONS: &[&str] = &[
    "page-sources",
    "page-requirements",
    "page-resolutions",
    "page-rules",
    "page-topics",
    "page-questions",
    "get-reviewed-resource",
    "read-document",
    "review-discussions",
    "review-discussion-messages",
];

/// The name in [`READ_OPERATIONS`] that equals `operation`.
pub fn read_operation(operation: &str) -> Option<&'static str> {
    READ_OPERATIONS
        .iter()
        .copied()
        .find(|name| *name == operation)
}
