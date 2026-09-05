mod contradiction;
mod dangling;
mod frontier;
pub mod graph_query;
mod model;
mod state_adapter;

use provenance_macros::rule;

pub use graph_query::{GapGraph, GraphQuery};
pub use model::{node_type_word, GapItem, GapKind};
pub use state_adapter::find_gaps;
pub(in crate::cache) use state_adapter::GraphRecords;

/// Computes gaps in stable policy order. This is also the seam for callers
/// that already hold graph records rather than a state store.
///
/// A rule counts as traced when its requirement is recorded as producing it.
/// A resolution may also produce it when a decision removed ambiguity, but
/// is not required. The health report reads the same join, so the two never
/// disagree.
#[rule("rule_graph_gaps")]
pub fn compute_gaps(graph: &GapGraph<'_>) -> Vec<GapItem> {
    let query = GraphQuery::new(graph);
    let mut gaps = Vec::new();
    frontier::add_requirement_gaps(&query, &mut gaps);
    frontier::add_source_gaps(&query, &mut gaps);
    dangling::add_reference_gaps(&query, &mut gaps);
    contradiction::add_gaps(&query, &mut gaps);
    frontier::add_question_gaps(&query, &mut gaps);
    frontier::add_topic_gaps(&query, &mut gaps);
    gaps
}

/// The record builders under `tests::fixtures` are the one place that knows
/// how to spell a bare graph record, so the cache-level tests reuse them
/// rather than growing a second set.
#[cfg(test)]
pub(in crate::cache) mod tests;
