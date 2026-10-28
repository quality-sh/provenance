//! A live record whose Requirement parents are all archived.
//!
//! Writes refuse a new link to an archived Requirement, and an archive
//! cascades to each child that has no live parent. A merge of two branches
//! can still join an archive on one branch with a new child on the other.
//! The gap report states that record; graph validation does not refuse it,
//! because a refusal there stops the reads and writes that repair it.

use super::{
    graph_query::GraphQuery,
    model::{GapItem, GapKind},
};
use provenance_core::{
    NodeType, QuestionStatus, RequirementStatus, ResolutionStatus, RuleStatus, StableId,
    TopicStatus,
};

/// One gap per live record whose Requirement parents are all archived.
/// A Boundary has no lifecycle status, so it is not a live dependent.
pub(super) fn add_gaps(query: &GraphQuery<'_, '_>, gaps: &mut Vec<GapItem>) {
    let graph = query.graph;
    for record in graph.requirements {
        let is_live = record.status != RequirementStatus::Archived;
        add(
            query,
            is_live,
            NodeType::Requirement,
            &record.id,
            &record.refines,
            gaps,
        );
    }
    for record in graph.resolutions {
        let is_live = !matches!(
            record.status,
            ResolutionStatus::Abandoned | ResolutionStatus::Rejected | ResolutionStatus::Archived
        );
        add(
            query,
            is_live,
            NodeType::Resolution,
            &record.id,
            &record.requirement_ids,
            gaps,
        );
    }
    for record in graph.rules {
        let is_live = record.status != RuleStatus::Archived;
        add(
            query,
            is_live,
            NodeType::Rule,
            &record.id,
            &record.requirement_ids,
            gaps,
        );
    }
    for record in graph.topics {
        let parents = std::slice::from_ref(&record.requirement_id);
        let is_live = record.status != TopicStatus::Archived;
        add(query, is_live, NodeType::Topic, &record.id, parents, gaps);
    }
    for record in graph.questions {
        let parents = std::slice::from_ref(&record.requirement_id);
        let is_live = record.status != QuestionStatus::Archived;
        add(
            query,
            is_live,
            NodeType::Question,
            &record.id,
            parents,
            gaps,
        );
    }
}

fn add(
    query: &GraphQuery<'_, '_>,
    is_live: bool,
    node_type: NodeType,
    id: &StableId,
    parents: &[StableId],
    gaps: &mut Vec<GapItem>,
) {
    if !is_live || parents.is_empty() || !parents.iter().all(|parent| archived(query, parent)) {
        return;
    }
    let mut gap = GapItem::new(
        GapKind::NoLiveParent,
        node_type,
        id,
        "every requirement parent is archived",
    )
    .with_related(NodeType::Requirement, &parents[0]);
    // A shaping gap names its Requirement so prime can group it.
    if matches!(node_type, NodeType::Topic | NodeType::Question) {
        gap = gap.with_requirement(&parents[0]);
    }
    gaps.push(gap);
}

/// A missing parent is a dangling reference, not an archived one.
fn archived(query: &GraphQuery<'_, '_>, id: &StableId) -> bool {
    query
        .graph
        .requirements
        .iter()
        .any(|record| record.id == *id && record.status == RequirementStatus::Archived)
}
