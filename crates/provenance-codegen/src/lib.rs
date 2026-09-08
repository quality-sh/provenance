//! Catalog-driven operation definitions and client type generation.
mod documents;
pub use documents::{component, documents};
mod corpus;
mod rust_types;
pub use corpus::corpus;
pub use rust_types::rust_types;

mod unions;

mod model_layout;
