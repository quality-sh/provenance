use super::{
    graph_query::GraphQuery,
    model::{GapItem, GapKind},
};
use provenance_core::{NodeType, Question, StableId};
use std::collections::BTreeSet;

/// A contradiction is a question that names both requirements. The
/// unordered pair is the gap identity; it is settled when the question
/// carries a resolution or either requirement lists the other in
/// `supersedes`.
pub(super) fn add_gaps(query: &GraphQuery<'_, '_>, gaps: &mut Vec<GapItem>) {
    let mut seen: BTreeSet<(&str, &str)> = BTreeSet::new();
    for question in query.graph.questions {
        let Some(other) = &question.contradicts else {
            continue;
        };
        if !query.requirement_exists(&question.requirement_id) || !query.requirement_exists(other) {
            continue;
        }
        let pair = ordered_pair(&question.requirement_id, other);
        if !seen.insert(pair) || is_resolved(query, question, other) {
            continue;
        }
        gaps.push(
            GapItem::new(
                GapKind::UnresolvedContradictsPair,
                NodeType::Requirement,
                &question.requirement_id,
                "unresolved `contradicts` pair",
            )
            .with_related(NodeType::Requirement, other),
        );
    }
}

fn is_resolved(query: &GraphQuery<'_, '_>, question: &Question, other: &StableId) -> bool {
    if question.resolution_id.is_some() {
        return true;
    }
    let supersedes =
        |left: &StableId, right: &StableId| {
            query.graph.requirements.iter().any(|requirement| {
                requirement.id == *left && requirement.supersedes.contains(right)
            })
        };
    supersedes(&question.requirement_id, other) || supersedes(other, &question.requirement_id)
}

fn ordered_pair<'a>(left: &'a StableId, right: &'a StableId) -> (&'a str, &'a str) {
    if left.as_str() <= right.as_str() {
        (left.as_str(), right.as_str())
    } else {
        (right.as_str(), left.as_str())
    }
}
