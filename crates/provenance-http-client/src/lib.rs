//! Connection-based client generated from the operation `OpenAPI` contract.
// Pinned Typify emits these conditional forms and explicit default helpers.
#[allow(
    clippy::if_not_else,
    clippy::missing_const_for_fn,
    clippy::derivable_impls,
    clippy::default_trait_access,
    clippy::large_enum_variant
)]
pub mod types {
    include!("generated/types.rs");
}
/// Closed enums for every declared operation parameter value set. A call
/// names one variant or passes no value, so an invented selector, direction,
/// or side cannot compile.
pub mod parameters {
    include!("generated/parameters.rs");
}
mod client {
    include!("generated/client.rs");
}
mod runtime;
pub use client::{HttpClient, OperationFailure, COMPATIBILITY, PROTOCOL_VERSION};
pub use runtime::{Error, ResponseFailure, MAX_RESPONSE_BYTES};
