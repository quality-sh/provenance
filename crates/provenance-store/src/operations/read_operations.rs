//! The read operations of one repository, as a port.
//!
//! The contract router sends each of these operations to the port of the
//! repository it serves, when the repository has one, and does not run the
//! operation itself. So an embedder can run the reads beside the read
//! database, in another process, and serve the contract routes from a
//! repository that has only its canonical state.
//!
//! Before the router gives a call to the port, it does all that it does for
//! an operation that it runs itself: it decodes the call, checks the
//! request, authorizes the context, waits for a work slot and prepares the
//! repository through its host access. After the port answers, the router
//! checks the answer against the operation: a success must have the shape
//! of the operation's success, and a failure must be one that the operation
//! declares, with its status. An answer that is not declared becomes the
//! `internal` failure.
//!
//! A request is the call of one operation as the contract dispatch takes
//! it: `{"context": {...}, "request": {...}}`. An answer is the encoded
//! success, or the failure that the operation declares, with its status.
//!
//! [`InProcessReadOperations`] runs each operation on one opened
//! repository, with the checks and the preparation of [`LocalRepository`].
//! It gives the answers that the router gives when it runs the operation
//! itself.
//!
//! [`LocalRepository`]: super::LocalRepository

use provenance_core::protocol::failure::ErasedFailure;
use serde_json::Value;
use std::fmt::Debug;
use std::future::Future;
use std::pin::Pin;

mod in_process;
pub use in_process::InProcessReadOperations;

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
/// messages of a record. None of them writes.
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
