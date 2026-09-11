pub mod cache;
pub mod canonical_digest;
pub mod code_refs;
pub mod dictionary_reference;
pub mod evidence_anchors;
pub mod graph_reference;
pub mod jsonl;
pub mod layout;
pub mod merge;
pub mod migrations;
pub mod operations;
pub mod publication;
pub mod review;
pub mod settings;
pub mod shards;
pub mod stale;
pub mod state_store;
pub mod statement_analysis;
mod test_probes;

/// The validator version that a completed projection rebuild records.
///
/// 1. Graph and ideation validation when catch-up reads changed units in place.
///
/// Increase this version when a validator change requires existing scopes to be checked again.
pub const VALIDATION_VERSION: u32 = 1;

pub mod write_error;
