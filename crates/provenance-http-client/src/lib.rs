//! Connection-based client generated from the operation `OpenAPI` contract.
// Pinned Typify emits these conditional forms and explicit default helpers.
#[allow(
    clippy::if_not_else,
    clippy::missing_const_for_fn,
    clippy::derivable_impls
)]
pub mod types {
    include!("generated/types.rs");
}
mod client {
    include!("generated/client.rs");
}
pub use client::{Error, HttpClient, OperationFailure, PROTOCOL_VERSION};
