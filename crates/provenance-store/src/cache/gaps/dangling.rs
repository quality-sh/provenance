use super::{
    graph_query::GraphQuery,
    model::{node_type_word, GapItem, GapKind},
};
use provenance_core::model::relations::{link_target, RelationOwner};
use provenance_core::{ArtifactLink, NodeType, StableId};

/// Every declared reference field of every owner kind, then the hand-walked
/// links and thread parents.
pub(super) fn add_reference_gaps(query: &GraphQuery<'_, '_>, gaps: &mut Vec<GapItem>) {
    add_declared_refs(query, query.graph.sources, gaps);
    add_declared_refs(query, query.graph.requirements, gaps);
    add_declared_refs(query, query.graph.resolutions, gaps);
    add_declared_refs(query, query.graph.rules, gaps);
    add_declared_refs(query, query.graph.topics, gaps);
    add_declared_refs(query, query.graph.questions, gaps);
    add_declared_refs(query, query.graph.boundaries, gaps);
    for topic in query.graph.topics {
        add_link_refs(query, NodeType::Topic, &topic.id, &topic.links, None, gaps);
    }
    for question in query.graph.questions {
        add_link_refs(
            query,
            NodeType::Question,
            &question.id,
            &question.links,
            Some(&question.requirement_id),
            gaps,
        );
    }
    add_thread_refs(query, gaps);
}

/// One gap per reference whose target is not in the scope, worded from
/// the declaration: `<relation> points at missing <kind> <id>`.
fn add_declared_refs<T: RelationOwner>(
    query: &GraphQuery<'_, '_>,
    records: &[T],
    gaps: &mut Vec<GapItem>,
) {
    for record in records {
        for (name, target) in record.references() {
            let decl = provenance_core::model::relations::declaration_of(T::relations(), name)
                .expect("references name declared fields");
            if query.node_exists(decl.target, target) {
                continue;
            }
            let mut gap = GapItem::new(
                GapKind::DanglingReference,
                T::OWNER,
                record.id(),
                format!(
                    "{name} points at missing {} {}",
                    node_type_word(decl.target),
                    target.as_str()
                ),
            )
            .with_related(decl.target, target);
            if let Some(requirement) = shaping_requirement(T::OWNER, record) {
                gap = gap.with_requirement(requirement);
            }
            gaps.push(gap);
        }
    }
}

/// A question's gap names its requirement so prime can group it.
fn shaping_requirement<T: RelationOwner>(owner: NodeType, record: &T) -> Option<&StableId> {
    if owner != NodeType::Question {
        return None;
    }
    record
        .references()
        .into_iter()
        .find(|(name, _)| *name == "requirement_id")
        .map(|(_, id)| id)
}

fn add_link_refs(
    query: &GraphQuery<'_, '_>,
    owner: NodeType,
    id: &StableId,
    links: &[ArtifactLink],
    requirement: Option<&StableId>,
    gaps: &mut Vec<GapItem>,
) {
    for link in links {
        let target = link_target(link);
        if query.node_exists(target, &link.target_id) {
            continue;
        }
        let mut gap = GapItem::new(
            GapKind::DanglingReference,
            owner,
            id,
            format!(
                "links points at missing {} {}",
                node_type_word(target),
                link.target_id.as_str()
            ),
        )
        .with_related(target, &link.target_id);
        if let Some(requirement) = requirement {
            gap = gap.with_requirement(requirement);
        }
        gaps.push(gap);
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
