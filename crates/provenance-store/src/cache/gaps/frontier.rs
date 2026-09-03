use super::{
    graph_query::GraphQuery,
    model::{GapItem, GapKind},
};
use provenance_core::{NodeType, QuestionStatus, RequirementStatus, TopicStatus};

pub(super) fn add_requirement_gaps(query: &GraphQuery<'_, '_>, gaps: &mut Vec<GapItem>) {
    for requirement in query.graph.requirements {
        let resolving = query.resolving_resolutions(&requirement.id);
        let resolved = requirement.status == RequirementStatus::Resolved;
        if requirement.domain_id.is_none() {
            gaps.push(GapItem::new(
                GapKind::MissingDomainId,
                NodeType::Requirement,
                &requirement.id,
                "requirement has no domain_id",
            ));
        }
        if !query.requirement_has_valid_source(requirement) {
            gaps.push(GapItem::new(
                GapKind::MissingSourceRefs,
                NodeType::Requirement,
                &requirement.id,
                "requirement has no source refs",
            ));
        }
        if resolved && resolving.is_empty() {
            gaps.push(GapItem::new(
                GapKind::NoResolvingDecision,
                NodeType::Requirement,
                &requirement.id,
                "resolved requirement has no resolving decision",
            ));
        }
        if (resolved || !resolving.is_empty())
            && query
                .produced_rules_for_requirement(&requirement.id)
                .is_empty()
        {
            gaps.push(GapItem::new(
                GapKind::NoProducedRules,
                NodeType::Requirement,
                &requirement.id,
                "requirement has no downstream rule",
            ));
        }
    }
}

pub(super) fn add_source_gaps(query: &GraphQuery<'_, '_>, gaps: &mut Vec<GapItem>) {
    for source in query.graph.sources {
        if !query.source_is_referenced(&source.id) {
            gaps.push(GapItem::new(
                GapKind::UnreferencedSource,
                NodeType::Source,
                &source.id,
                "no requirement references this source",
            ));
        }
    }
}

/// A question that names a contradiction is reported as the pair gap, not
/// as an open question as well.
pub(super) fn add_question_gaps(query: &GraphQuery<'_, '_>, gaps: &mut Vec<GapItem>) {
    for question in query.graph.questions.iter().filter(|question| {
        question.contradicts.is_none()
            && matches!(
                question.status,
                QuestionStatus::Open | QuestionStatus::BlockedOnHuman
            )
    }) {
        let reason = match question.status {
            QuestionStatus::Open => "open question",
            QuestionStatus::BlockedOnHuman => "blocked_on_human question",
            QuestionStatus::Answered => unreachable!("answered questions are filtered out"),
        };
        gaps.push(
            GapItem::new(
                GapKind::OpenQuestion,
                NodeType::Question,
                &question.id,
                reason,
            )
            .with_requirement(&question.requirement_id),
        );
    }
}

pub(super) fn add_topic_gaps(query: &GraphQuery<'_, '_>, gaps: &mut Vec<GapItem>) {
    for topic in query
        .graph
        .topics
        .iter()
        .filter(|topic| topic.status == TopicStatus::Open)
    {
        gaps.push(
            GapItem::new(
                GapKind::UnexploredTopic,
                NodeType::Topic,
                &topic.id,
                "unexplored topic",
            )
            .with_requirement(&topic.requirement_id),
        );
    }
}
