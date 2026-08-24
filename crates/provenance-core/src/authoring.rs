//! The pure authoring kernel.
//!
//! One home for immutable construction, language-neutral structural
//! validation, declaration-address construction, and canonical assembly
//! of typed-spec documents. The kernel performs no file, environment,
//! process, clock, repository, scanner, or store access; the store calls
//! into it at ingestion, never the other way round.

pub mod addresses;
mod builders;
mod checks;
mod document;
mod error;
mod handles;

pub use builders::{
    requirement, rule, source, spec, RequirementBuilder, RuleBuilder, SourceBuilder, SpecBuilder,
};
pub use checks::{
    normalize_rule_relationships, validate_references, DeclarationChecker, RuleChecker,
};
pub use document::SpecDocument;
pub use error::AuthoringError;
pub use handles::{RequirementHandle, RuleHandle, SourceHandle, SpecHandles};

#[cfg(test)]
mod tests;
