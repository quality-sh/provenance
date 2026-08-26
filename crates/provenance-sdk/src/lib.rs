//! The thin Rust SDK facade.
//!
//! Re-exports plus three frontend residues: verification orchestration,
//! the macro projection, and environment settings. No materialization,
//! no structural validation, no address logic, and no semantic layer
//! lives here; provenance-core owns authoring and provenance-store owns
//! identity, planning, and mutation. The parity ledger is PARITY.md.

mod macros;
mod settings;
mod verify;

pub use provenance_core::authoring::{
    requirement, rule, source, spec, AuthoringError, RequirementBuilder, RequirementHandle,
    RuleBuilder, RuleHandle, SourceBuilder, SpecBuilder, SpecDocument, SpecHandles,
};
pub use provenance_core::protocol::{
    TypedAdoptionTarget, TypedDeclarationKind, TypedImplementationInput, TypedRequirementInput,
    TypedRuleInput, TypedSourceInput, TypedSpecInput,
};
pub use provenance_core::{EngineInfo, ScopeId, SourceType, StableId, SDK_PROTOCOL_VERSION};
pub use provenance_macros::{rule, verifies};
pub use provenance_store::operations;
pub use provenance_store::operations::{AffectedRule, TypedSpecPlan};
pub use provenance_store::state_store::TypedSpecResult;

pub use macros::identifier_matches_key;
#[doc(hidden)]
pub use macros::implemented_by_package_path;
pub use settings::Settings;
pub use verify::{verify, VerifyTarget};
