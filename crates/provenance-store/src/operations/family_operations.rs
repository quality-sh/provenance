//! The list, page and member operations of the record families.
//!
//! The verification and proposal resources use the same shapes. `provenance-contract` names
//! these types when it registers routes and dispatch entries.

#[cfg(feature = "host")]
pub use super::catalog::verification_resources::*;
pub use super::catalog::{resource_lists::*, resource_members::*, resource_pages::*};
