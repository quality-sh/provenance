//! Typed authoring documents that delete records.
//!
//! `deletion`: a document that deletes a Requirement removes its id from the
//! records that cite it. It has no Rule binding, because the graph has no Rule
//! for the typed deletion cascade. `plan_apply` and `cascade_refusals`: the
//! plan shows each deletion that the apply refuses. `landed_disposition`: a
//! landed Disposition that names no record is a fault of the stored records.
#[path = "cli_sdk_lifecycle/cascade_refusals.rs"]
mod cascade_refusals;
#[path = "cli_sdk_lifecycle/deletion.rs"]
mod deletion;
#[path = "cli_sdk_lifecycle/landed_disposition.rs"]
mod landed_disposition;
#[path = "cli_sdk_lifecycle/plan_apply.rs"]
mod plan_apply;
mod porcelain_authoring_support;
