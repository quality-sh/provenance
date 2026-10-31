//! `GraphRecords` built from records a caller holds: the gaps and the
//! neighbours come out the same as from a hand-built graph, with no store.

use crate::cache::gaps::tests::fixtures as records;
use crate::cache::{compute_gaps, GapKind, GraphRecords};
use provenance_core::model::relations::{flow_neighbors, related_nodes, RecordFront};
use provenance_core::{NodeType, ScopeId, SourceReference};
use provenance_macros::verifies;

fn held_records() -> GraphRecords {
    let mut cited = records::requirement("req_cited");
    cited.source_refs.push(SourceReference {
        source_id: records::sid("source_award"),
        clause: None,
    });
    let mut refining = records::requirement("req_refining");
    refining.refines.push(records::sid("req_cited"));
    let mut rule = records::rule("rule_cited");
    rule.requirement_ids.push(records::sid("req_cited"));
    GraphRecords {
        sources: vec![
            records::source("source_award"),
            records::source("source_lone"),
        ],
        requirements: vec![cited, refining],
        rules: vec![rule],
        ..GraphRecords::default()
    }
}

#[test]
#[verifies("rule_graph_gaps", examples)]
fn held_records_give_the_gaps_of_the_same_graph() {
    let held = held_records();
    let scope = ScopeId::new("default").unwrap();

    let gaps = compute_gaps(&held.graph(&scope));

    let expected = records::compute_for(
        &held.sources,
        &held.requirements,
        &[],
        &held.rules,
        &[],
        &[],
        &[],
    );
    assert_eq!(gaps, expected);
    assert_eq!(records::count_kind(&gaps, GapKind::UnreferencedSource), 1);
}

#[test]
#[verifies("rule_neighbors_one_per_relation_direction_endpoint", examples)]
fn held_records_give_the_neighbours_of_a_hand_built_front() {
    let held = held_records();
    let scope = ScopeId::new("default").unwrap();
    let graph = held.graph(&scope);
    let front = graph.front();
    let hand_built = RecordFront {
        sources: &held.sources,
        requirements: &held.requirements,
        resolutions: &[],
        rules: &held.rules,
        topics: &[],
        questions: &[],
        domains: &[],
        boundaries: &[],
    };
    let cited = records::sid("req_cited");

    let related = related_nodes(&front, NodeType::Requirement, &cited);

    assert_eq!(
        related,
        related_nodes(&hand_built, NodeType::Requirement, &cited)
    );
    assert_eq!(related.len(), 3);
    for downstream in [true, false] {
        assert_eq!(
            flow_neighbors(&front, NodeType::Requirement, &cited, downstream),
            flow_neighbors(&hand_built, NodeType::Requirement, &cited, downstream)
        );
    }
}
