//! The gap rule: `compute_gaps` decides what counts as an unfinished spot
//! in a graph. Proven per kind by minimal graphs, and by properties over
//! complete graphs.

use super::super::*;
use super::fixtures::*;
use crate::cache::gaps::tests::fixtures as records;
use provenance_core::{
    Domain, Question, QuestionStatus, Requirement, RequirementStatus, Resolution, ResolutionStatus,
    Rule, Source, SourceReference, Topic, TopicStatus,
};
use provenance_macros::verifies;

const ANCHOR_SOURCE: &str = "source_anchor";
const ANCHOR_DOMAIN: &str = "domain_shaping";

/// The variant list is derived from an exhaustive match so that adding a
/// `GapKind` fails compilation until the new variant joins the chain, which
/// keeps the per-kind exhaustion below complete.
pub(super) fn all_gap_kinds() -> Vec<GapKind> {
    let mut all = vec![GapKind::MissingDomainId];
    while let Some(next) = match all.last().unwrap() {
        GapKind::MissingDomainId => Some(GapKind::MissingSourceRefs),
        GapKind::MissingSourceRefs => Some(GapKind::NoResolvingDecision),
        GapKind::NoResolvingDecision => Some(GapKind::NoProducedRules),
        GapKind::NoProducedRules => Some(GapKind::UnreferencedSource),
        GapKind::UnreferencedSource => Some(GapKind::DanglingReference),
        GapKind::DanglingReference => Some(GapKind::UnresolvedContradictsPair),
        GapKind::UnresolvedContradictsPair => Some(GapKind::OpenQuestion),
        GapKind::OpenQuestion => Some(GapKind::UnexploredTopic),
        GapKind::UnexploredTopic => None,
    } {
        all.push(next);
    }
    all
}

/// Owned records fed straight to `compute_gaps`, bypassing the state store so
/// a case can hold exactly the records it means to hold and nothing else.
#[derive(Default)]
struct HandBuiltGraph {
    sources: Vec<Source>,
    requirements: Vec<Requirement>,
    resolutions: Vec<Resolution>,
    rules: Vec<Rule>,
    topics: Vec<Topic>,
    questions: Vec<Question>,
    domains: Vec<Domain>,
}

impl HandBuiltGraph {
    fn gaps(&self) -> Vec<GapItem> {
        records::compute_for(
            &self.sources,
            &self.requirements,
            &self.resolutions,
            &self.rules,
            &self.topics,
            &self.questions,
            &self.domains,
        )
    }

    /// The shaping domain every settled requirement names.
    fn with_domain(mut self) -> Self {
        self.domains.push(records::domain(ANCHOR_DOMAIN));
        self
    }
}

/// A requirement carrying a source ref but no domain.
fn sourced_requirement(id: &str, source_id: &str, status: RequirementStatus) -> Requirement {
    Requirement {
        status,
        source_refs: vec![SourceReference {
            source_id: sid(source_id),
            clause: None,
        }],
        ..records::requirement(id)
    }
}

/// A requirement with nothing missing on its own account: domain assigned and
/// a live source behind it.
fn settled_requirement(id: &str, source_id: &str, status: RequirementStatus) -> Requirement {
    Requirement {
        domain_id: Some(sid(ANCHOR_DOMAIN)),
        ..sourced_requirement(id, source_id, status)
    }
}

fn resolution_of(id: &str, requirement: &str, status: ResolutionStatus) -> Resolution {
    Resolution {
        status,
        requirement_ids: vec![sid(requirement)],
        ..records::resolution(id)
    }
}

fn rule_of(id: &str, requirements: &[&str], resolutions: &[&str]) -> Rule {
    Rule {
        requirement_ids: requirements.iter().map(|id| sid(id)).collect(),
        resolution_ids: resolutions.iter().map(|id| sid(id)).collect(),
        ..records::rule(id)
    }
}

/// One minimal graph per gap kind. The match is exhaustive, so a new
/// `GapKind` cannot be added without stating the smallest graph that opens it.
#[allow(clippy::too_many_lines)]
fn minimal_graph_for(kind: GapKind) -> HandBuiltGraph {
    let graph = match kind {
        GapKind::MissingDomainId => HandBuiltGraph {
            sources: vec![records::source(ANCHOR_SOURCE)],
            requirements: vec![sourced_requirement(
                "req_no_domain",
                ANCHOR_SOURCE,
                RequirementStatus::Active,
            )],
            ..HandBuiltGraph::default()
        },
        GapKind::MissingSourceRefs => HandBuiltGraph {
            requirements: vec![Requirement {
                domain_id: Some(sid(ANCHOR_DOMAIN)),
                ..records::requirement("req_no_source")
            }],
            ..HandBuiltGraph::default()
        },
        // Resolved, and with a downstream rule of its own, but nothing
        // recorded as having decided it.
        GapKind::NoResolvingDecision => HandBuiltGraph {
            sources: vec![records::source(ANCHOR_SOURCE)],
            requirements: vec![settled_requirement(
                "req_resolved",
                ANCHOR_SOURCE,
                RequirementStatus::Resolved,
            )],
            rules: vec![rule_of("rule_direct", &["req_resolved"], &[])],
            ..HandBuiltGraph::default()
        },
        // Decided, but the decision produced nothing.
        GapKind::NoProducedRules => HandBuiltGraph {
            sources: vec![records::source(ANCHOR_SOURCE)],
            requirements: vec![settled_requirement(
                "req_decided",
                ANCHOR_SOURCE,
                RequirementStatus::Active,
            )],
            resolutions: vec![resolution_of(
                "res_decided",
                "req_decided",
                ResolutionStatus::Draft,
            )],
            ..HandBuiltGraph::default()
        },
        GapKind::UnreferencedSource => HandBuiltGraph {
            sources: vec![records::source("source_unused")],
            ..HandBuiltGraph::default()
        },
        // An explored topic hung off a requirement that is not there.
        GapKind::DanglingReference => HandBuiltGraph {
            topics: vec![records::topic_for(
                "topic_dangling",
                "req_missing",
                TopicStatus::Explored,
            )],
            ..HandBuiltGraph::default()
        },
        // A question names the pair and nothing has settled it.
        GapKind::UnresolvedContradictsPair => HandBuiltGraph {
            sources: vec![records::source(ANCHOR_SOURCE)],
            requirements: vec![
                settled_requirement("req_left", ANCHOR_SOURCE, RequirementStatus::Active),
                settled_requirement("req_right", ANCHOR_SOURCE, RequirementStatus::Active),
            ],
            topics: vec![records::topic_for(
                "topic_pair",
                "req_left",
                TopicStatus::Explored,
            )],
            questions: vec![Question {
                contradicts: Some(sid("req_right")),
                ..records::question_for(
                    "question_pair",
                    "topic_pair",
                    "req_left",
                    QuestionStatus::Open,
                )
            }],
            ..HandBuiltGraph::default()
        },
        GapKind::OpenQuestion => HandBuiltGraph {
            sources: vec![records::source(ANCHOR_SOURCE)],
            requirements: vec![settled_requirement(
                "req_asked",
                ANCHOR_SOURCE,
                RequirementStatus::Active,
            )],
            topics: vec![records::topic_for(
                "topic_asked",
                "req_asked",
                TopicStatus::Explored,
            )],
            questions: vec![records::question_for(
                "question_open",
                "topic_asked",
                "req_asked",
                QuestionStatus::Open,
            )],
            ..HandBuiltGraph::default()
        },
        GapKind::UnexploredTopic => HandBuiltGraph {
            sources: vec![records::source(ANCHOR_SOURCE)],
            requirements: vec![settled_requirement(
                "req_open_topic",
                ANCHOR_SOURCE,
                RequirementStatus::Active,
            )],
            topics: vec![records::topic_for(
                "topic_open",
                "req_open_topic",
                TopicStatus::Open,
            )],
            ..HandBuiltGraph::default()
        },
    };
    graph.with_domain()
}

/// A graph with nothing left unfinished: every requirement has a domain and
/// live source, its approved decision names it and its rule names both.
/// Every topic is explored and every question answered.
fn complete_graph(chains: usize) -> HandBuiltGraph {
    let mut graph = HandBuiltGraph::default().with_domain();
    for index in 0..chains {
        let source = format!("source_{index}");
        let requirement = format!("req_{index}");
        let resolution = format!("res_{index}");
        let rule = format!("rule_{index}");
        let topic = format!("topic_{index}");
        let question = format!("question_{index}");
        graph.sources.push(records::source(&source));
        graph.requirements.push(settled_requirement(
            &requirement,
            &source,
            RequirementStatus::Resolved,
        ));
        graph.resolutions.push(resolution_of(
            &resolution,
            &requirement,
            ResolutionStatus::Approved,
        ));
        graph
            .rules
            .push(rule_of(&rule, &[&requirement], &[&resolution]));
        graph.topics.push(records::topic_for(
            &topic,
            &requirement,
            TopicStatus::Explored,
        ));
        graph.questions.push(records::question_for(
            &question,
            &topic,
            &requirement,
            QuestionStatus::Answered,
        ));
    }
    graph
}

#[test]
#[verifies("rule_graph_gaps", exhaustion)]
fn every_gap_kind_has_a_minimal_graph_that_produces_exactly_it() {
    for kind in all_gap_kinds() {
        let gaps = minimal_graph_for(kind).gaps();
        assert_eq!(
            gaps.iter().map(|gap| gap.kind).collect::<Vec<_>>(),
            vec![kind],
            "the minimal graph for {kind:?} does not produce exactly that gap; got {gaps:#?}"
        );
    }
}

#[test]
#[verifies("rule_graph_gaps", property)]
fn a_complete_graph_has_no_gaps() {
    for chains in 0..=6 {
        let graph = complete_graph(chains);
        let gaps = graph.gaps();
        assert!(
            gaps.is_empty(),
            "a complete graph of {chains} chains reports gaps: {gaps:#?}"
        );
    }
}

/// The links a complete chain needs: the resolution naming its requirement
/// and the requirement citing its source. The rule's own lists are the
/// validator's subject, not the gap policy's.
#[test]
#[verifies("rule_graph_gaps", property)]
fn removing_any_single_required_link_from_a_complete_graph_opens_a_gap() {
    for chains in 1..=4 {
        for index in 0..chains {
            let mut graph = complete_graph(chains);
            graph.resolutions[index].requirement_ids.clear();
            let gaps = graph.gaps();
            assert!(
                !gaps.is_empty(),
                "clearing the requirement of resolution {index} from a complete graph \
                 of {chains} chains left no gap"
            );
            let mut graph = complete_graph(chains);
            graph.requirements[index].source_refs.clear();
            let gaps = graph.gaps();
            assert!(
                !gaps.is_empty(),
                "dropping the source ref of requirement {index} from a complete graph \
                 of {chains} chains left no gap"
            );
        }
    }
}

/// A rule names its requirement; naming a resolution as well is optional,
/// and a rule reached only through a resolution still counts as produced.
#[test]
#[verifies("rule_graph_gaps", examples)]
fn a_rule_reaches_its_requirement_directly_or_through_a_resolution() {
    let mut requirement_only = complete_graph(1);
    requirement_only.rules[0].resolution_ids.clear();
    assert!(requirement_only.gaps().is_empty());

    let mut through_resolution = complete_graph(1);
    through_resolution.rules[0].requirement_ids.clear();
    assert!(through_resolution.gaps().is_empty());

    let mut unattached = complete_graph(1);
    unattached.rules[0].requirement_ids.clear();
    unattached.rules[0].resolution_ids.clear();
    assert_eq!(
        unattached
            .gaps()
            .iter()
            .map(|gap| gap.kind)
            .collect::<Vec<_>>(),
        vec![GapKind::NoProducedRules]
    );
}
