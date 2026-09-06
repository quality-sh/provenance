use super::{
    graph_query::GraphQuery,
    model::{node_type_word, GapItem, GapKind},
};
use provenance_core::{IdeationTarget, NodeType, StableId};
use provenance_macros::rule;

/// A contribution or synthesis packet already in the scope whose `target`
/// names no record is reported as a dangling reference and refused
/// nowhere. As for a thread parent, the gap's subject is the missing
/// record and the reason names the owner.
#[rule("rule_old_dangling_ideation_target_is_a_gap")]
pub(super) fn add_target_gaps(query: &GraphQuery<'_, '_>, gaps: &mut Vec<GapItem>) {
    for contribution in query.graph.contributions {
        add_missing_target(
            query,
            "contribution",
            &contribution.id,
            &contribution.target,
            gaps,
        );
    }
    for packet in query.graph.synthesis_packets {
        add_missing_target(query, "synthesis packet", &packet.id, &packet.target, gaps);
    }
}

fn add_missing_target(
    query: &GraphQuery<'_, '_>,
    owner: &str,
    owner_id: &StableId,
    target: &IdeationTarget,
    gaps: &mut Vec<GapItem>,
) {
    let kind = NodeType::from(target.artifact_type);
    if query.node_exists(kind, &target.artifact_id) {
        return;
    }
    gaps.push(GapItem::new(
        GapKind::DanglingReference,
        kind,
        &target.artifact_id,
        format!(
            "{owner} {} target points at missing {} {}",
            owner_id.as_str(),
            node_type_word(kind),
            target.artifact_id.as_str()
        ),
    ));
}
