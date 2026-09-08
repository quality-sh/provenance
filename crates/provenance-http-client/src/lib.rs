//! Connection-based client generated from the operation `OpenAPI` contract.
// Typify uses this conditional form in generated conversion implementations.
#[allow(clippy::if_not_else)]
pub mod types {
    include!("generated/types.rs");
}
mod client {
    include!("generated/client.rs");
}
pub use client::{Error, HttpClient, PROTOCOL_VERSION};
