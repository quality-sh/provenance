//! A typed authoring document that deletes a Requirement removes its id from
//! the records that cite it.
//!
//! This test has no Rule binding, because the graph has no Rule for the typed
//! deletion cascade yet.
#[path = "cli_sdk_lifecycle/deletion.rs"]
mod deletion;
mod porcelain_authoring_support;
