//! Typed authoring documents that delete records.
//!
//! `deletion`: a document that deletes a Requirement removes its id from the
//! records that cite it. It has no Rule binding, because the graph has no Rule
//! for the typed deletion cascade. `plan_apply`: the plan shows each deletion
//! that the apply refuses.
#[path = "cli_sdk_lifecycle/deletion.rs"]
mod deletion;
#[path = "cli_sdk_lifecycle/plan_apply.rs"]
mod plan_apply;
mod porcelain_authoring_support;
