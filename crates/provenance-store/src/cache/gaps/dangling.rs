use super::{
    graph_query::GraphQuery,
    model::{node_type_word, GapItem, GapKind},
};
use provenance_core::{Edge, EdgeType, NodeType, StableId};

pub(super) fn add_reference_gaps(query: &GraphQuery<'_, '_>, gaps: &mut Vec<GapItem>) {
    add_requirement_source_refs(query, gaps);
    add_source_refs(query, gaps);
    add_resolution_refs(query, gaps);
    add_topic_refs(query, gaps);
    add_question_refs(query, gaps);
    add_thread_refs(query, gaps);
    add_edge_refs(query, gaps);
    add_ideation_target_refs(query, gaps);
}

/// The ideation-target check: every ideation record names a target in the
/// superset vocabulary, and a target that does not resolve surfaces as a
/// typed gap item rather than silently dangling.
fn add_ideation_target_refs(query: &GraphQuery<'_, '_>, gaps: &mut Vec<GapItem>) {
    for (subject, target) in query.graph.ideation_targets {
        if !query.ideation_target_exists(target) {
            gaps.push(
                GapItem::new(
                    GapKind::DanglingReference,
                    target.node_type(),
                    &target.artifact_id,
                    format!(
                        "{subject} points at missing target {}",
                        target.artifact_id.as_str()
                    ),
                )
                .with_related(target.node_type(), &target.artifact_id),
            );
        }
    }
}

fn add_requirement_source_refs(query: &GraphQuery<'_, '_>, gaps: &mut Vec<GapItem>) {
    for requirement in query.graph.requirements {
        for reference in &requirement.source_refs {
            if !query.source_exists(&reference.source_id) {
                gaps.push(
                    GapItem::new(
                        GapKind::DanglingReference,
                        NodeType::Requirement,
                        &requirement.id,
                        format!(
                            "source ref points at missing source {}",
                            reference.source_id.as_str()
                        ),
                    )
                    .with_related(NodeType::Source, &reference.source_id),
                );
            }
        }
    }
}

fn add_source_refs(query: &GraphQuery<'_, '_>, gaps: &mut Vec<GapItem>) {
    for source in query.graph.sources {
        if let Some(id) = &source.superseded_by {
            if !query.source_exists(id) {
                gaps.push(
                    GapItem::new(
                        GapKind::DanglingReference,
                        NodeType::Source,
                        &source.id,
                        format!("superseded by missing source {}", id.as_str()),
                    )
                    .with_related(NodeType::Source, id),
                );
            }
        }
    }
}

fn add_resolution_refs(query: &GraphQuery<'_, '_>, gaps: &mut Vec<GapItem>) {
    for resolution in query.graph.resolutions {
        if let Some(id) = &resolution.superseded_by {
            if !query.resolution_exists(id) {
                gaps.push(
                    GapItem::new(
                        GapKind::DanglingReference,
                        NodeType::Resolution,
                        &resolution.id,
                        format!("superseded by missing resolution {}", id.as_str()),
                    )
                    .with_related(NodeType::Resolution, id),
                );
            }
        }
    }
}

fn add_topic_refs(query: &GraphQuery<'_, '_>, gaps: &mut Vec<GapItem>) {
    for topic in query.graph.topics {
        if !query.requirement_exists(&topic.requirement_id) {
            gaps.push(
                GapItem::new(
                    GapKind::DanglingReference,
                    NodeType::Topic,
                    &topic.id,
                    format!(
                        "topic points at missing requirement {}",
                        topic.requirement_id.as_str()
                    ),
                )
                .with_related(NodeType::Requirement, &topic.requirement_id),
            );
        }
    }
}

fn add_question_refs(query: &GraphQuery<'_, '_>, gaps: &mut Vec<GapItem>) {
    for question in query.graph.questions {
        if !query.topic_exists(&question.topic_id) {
            gaps.push(
                GapItem::new(
                    GapKind::DanglingReference,
                    NodeType::Question,
                    &question.id,
                    format!(
                        "question points at missing topic {}",
                        question.topic_id.as_str()
                    ),
                )
                .with_related(NodeType::Topic, &question.topic_id)
                .with_requirement(&question.requirement_id),
            );
        }
        if !query.requirement_exists(&question.requirement_id) {
            gaps.push(
                GapItem::new(
                    GapKind::DanglingReference,
                    NodeType::Question,
                    &question.id,
                    format!(
                        "question points at missing requirement {}",
                        question.requirement_id.as_str()
                    ),
                )
                .with_related(NodeType::Requirement, &question.requirement_id),
            );
        }
        if let Some(id) = &question.resolution_id {
            if !query.resolution_exists(id) {
                gaps.push(
                    GapItem::new(
                        GapKind::DanglingReference,
                        NodeType::Question,
                        &question.id,
                        format!("question points at missing resolution {}", id.as_str()),
                    )
                    .with_related(NodeType::Resolution, id)
                    .with_requirement(&question.requirement_id),
                );
            }
        }
    }
}

fn add_thread_refs(query: &GraphQuery<'_, '_>, gaps: &mut Vec<GapItem>) {
    for thread in query.graph.threads {
        if !query.node_exists(thread.parent.node_type, &thread.parent.node_id) {
            gaps.push(GapItem::new(
                GapKind::DanglingReference,
                thread.parent.node_type,
                &thread.parent.node_id,
                format!(
                    "thread {} points at missing {} {}",
                    thread.id.as_str(),
                    node_type_word(thread.parent.node_type),
                    thread.parent.node_id.as_str()
                ),
            ));
        }
    }
}

fn add_edge_refs(query: &GraphQuery<'_, '_>, gaps: &mut Vec<GapItem>) {
    for edge in query.edges() {
        let from_exists = query.node_exists(edge.from_type, &edge.from_id);
        let to_exists = query.node_exists(edge.to_type, &edge.to_id);
        if !from_exists {
            add_edge_endpoint_gap(
                gaps,
                edge,
                (edge.from_type, &edge.from_id),
                (edge.to_type, &edge.to_id),
                to_exists,
                "from",
            );
        }
        if !to_exists {
            add_edge_endpoint_gap(
                gaps,
                edge,
                (edge.to_type, &edge.to_id),
                (edge.from_type, &edge.from_id),
                from_exists,
                "to",
            );
        }
    }
}

fn add_edge_endpoint_gap(
    gaps: &mut Vec<GapItem>,
    edge: &Edge,
    missing: (NodeType, &StableId),
    other: (NodeType, &StableId),
    other_exists: bool,
    direction: &str,
) {
    let anchor = if other_exists { other } else { missing };
    let related = if other_exists { missing } else { other };
    gaps.push(
        GapItem::new(
            GapKind::DanglingReference,
            anchor.0,
            anchor.1,
            format!(
                "{} edge {} points {direction} missing {} {}",
                edge_type_word(edge.edge_type),
                edge.id.as_str(),
                node_type_word(missing.0),
                missing.1.as_str()
            ),
        )
        .with_related(related.0, related.1),
    );
}

const fn edge_type_word(edge_type: EdgeType) -> &'static str {
    match edge_type {
        EdgeType::References => "references",
        EdgeType::RefinesInto => "refines_into",
        EdgeType::DependsOn => "depends_on",
        EdgeType::Contradicts => "contradicts",
        EdgeType::Supersedes => "supersedes",
        EdgeType::Needs => "needs",
        EdgeType::Resolves => "resolves",
        EdgeType::Spawns => "spawns",
        EdgeType::Produces => "produces",
    }
}
