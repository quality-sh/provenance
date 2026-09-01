use super::model::node_type_word;
use provenance_core::{
    Boundary, Domain, Edge, EdgeType, NodeType, Question, Requirement, Resolution, Rule, ScopeId,
    Source, StableId, Thread, Topic,
};
use std::collections::BTreeSet;

/// A record type that may produce a rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleProducer {
    Requirement,
    Resolution,
}

impl RuleProducer {
    /// Producers required for a rule to have complete traceability.
    ///
    /// A resolution may produce a rule when a decision removed ambiguity,
    /// but a rule always refines a requirement directly.
    pub const REQUIRED: [Self; 1] = [Self::Requirement];

    pub const fn node_type(self) -> NodeType {
        match self {
            Self::Requirement => NodeType::Requirement,
            Self::Resolution => NodeType::Resolution,
        }
    }

    pub const fn word(self) -> &'static str {
        node_type_word(self.node_type())
    }
}

pub struct GapGraph<'a> {
    pub scope: &'a ScopeId,
    pub sources: &'a [Source],
    pub requirements: &'a [Requirement],
    pub resolutions: &'a [Resolution],
    pub rules: &'a [Rule],
    pub topics: &'a [Topic],
    pub questions: &'a [Question],
    pub edges: &'a [Edge],
    pub threads: &'a [Thread],
    pub domains: &'a [Domain],
    pub boundaries: &'a [Boundary],
}

/// Read-only joins over a [`GapGraph`].
///
/// Gap policy is written against these helpers, and they are the single home
/// for the traversals the wiki assembler needs too, so both readers answer
/// "what resolves this?" and "what did this produce?" the same way.
pub struct GraphQuery<'a, 'graph> {
    pub graph: &'a GapGraph<'graph>,
}

impl<'a, 'graph> GraphQuery<'a, 'graph> {
    pub const fn new(graph: &'a GapGraph<'graph>) -> Self {
        Self { graph }
    }

    pub fn edges(&self) -> impl Iterator<Item = &Edge> {
        let scope = self.graph.scope;
        self.graph
            .edges
            .iter()
            .filter(move |edge| edge.scope_id == *scope)
    }

    pub fn edge_exists(
        &self,
        edge_type: EdgeType,
        from_type: NodeType,
        from_id: &StableId,
        to_type: NodeType,
        to_id: &StableId,
    ) -> bool {
        self.edges().any(|edge| {
            edge.edge_type == edge_type
                && edge.from_type == from_type
                && edge.from_id == *from_id
                && edge.to_type == to_type
                && edge.to_id == *to_id
        })
    }

    pub fn source_exists(&self, id: &StableId) -> bool {
        self.graph.sources.iter().any(|source| source.id == *id)
    }

    pub fn requirement_exists(&self, id: &StableId) -> bool {
        self.graph
            .requirements
            .iter()
            .any(|requirement| requirement.id == *id)
    }

    pub fn resolution_exists(&self, id: &StableId) -> bool {
        self.graph
            .resolutions
            .iter()
            .any(|resolution| resolution.id == *id)
    }

    pub fn topic_exists(&self, id: &StableId) -> bool {
        self.graph.topics.iter().any(|topic| topic.id == *id)
    }

    pub fn node_exists(&self, node_type: NodeType, id: &StableId) -> bool {
        match node_type {
            NodeType::Source => self.source_exists(id),
            NodeType::Requirement => self.requirement_exists(id),
            NodeType::Resolution => self.resolution_exists(id),
            NodeType::Rule => self.graph.rules.iter().any(|rule| rule.id == *id),
            NodeType::Topic => self.topic_exists(id),
            NodeType::Question => self
                .graph
                .questions
                .iter()
                .any(|question| question.id == *id),
            NodeType::Domain => self.graph.domains.iter().any(|domain| domain.id == *id),
            NodeType::Boundary => self
                .graph
                .boundaries
                .iter()
                .any(|boundary| boundary.id == *id),
        }
    }

    pub fn resolving_resolutions(&self, requirement_id: &StableId) -> Vec<&'graph Resolution> {
        self.graph
            .resolutions
            .iter()
            .filter(|resolution| {
                self.edge_exists(
                    EdgeType::Resolves,
                    NodeType::Resolution,
                    &resolution.id,
                    NodeType::Requirement,
                    requirement_id,
                )
            })
            .collect()
    }

    pub fn resolution_resolves_any_requirement(&self, resolution_id: &StableId) -> bool {
        self.graph.requirements.iter().any(|requirement| {
            self.edge_exists(
                EdgeType::Resolves,
                NodeType::Resolution,
                resolution_id,
                NodeType::Requirement,
                &requirement.id,
            )
        })
    }

    pub fn produced_rules_for_requirement(&self, requirement_id: &StableId) -> Vec<&'graph Rule> {
        let resolution_ids: BTreeSet<&str> = self
            .resolving_resolutions(requirement_id)
            .into_iter()
            .map(|resolution| resolution.id.as_str())
            .collect();
        self.graph
            .rules
            .iter()
            .filter(|rule| {
                self.edges().any(|edge| {
                    edge.edge_type == EdgeType::Produces
                        && edge.to_type == NodeType::Rule
                        && edge.to_id == rule.id
                        && ((edge.from_type == NodeType::Requirement
                            && edge.from_id == *requirement_id)
                            || (edge.from_type == NodeType::Resolution
                                && resolution_ids.contains(edge.from_id.as_str())))
                })
            })
            .collect()
    }

    pub fn produced_rules_for_resolution(&self, resolution_id: &StableId) -> Vec<&'graph Rule> {
        self.graph
            .rules
            .iter()
            .filter(|rule| {
                self.edge_exists(
                    EdgeType::Produces,
                    NodeType::Resolution,
                    resolution_id,
                    NodeType::Rule,
                    &rule.id,
                )
            })
            .collect()
    }

    /// The requirements recorded as producing this rule. A `produces` edge
    /// from a record that is not in the scope is a dangling reference, not a
    /// producer, so the join runs from the records that exist.
    pub fn producing_requirements(&self, rule_id: &StableId) -> Vec<&'graph Requirement> {
        self.graph
            .requirements
            .iter()
            .filter(|requirement| {
                self.edge_exists(
                    EdgeType::Produces,
                    NodeType::Requirement,
                    &requirement.id,
                    NodeType::Rule,
                    rule_id,
                )
            })
            .collect()
    }

    /// The resolutions recorded as producing this rule, on the same terms as
    /// [`Self::producing_requirements`].
    pub fn producing_resolutions(&self, rule_id: &StableId) -> Vec<&'graph Resolution> {
        self.graph
            .resolutions
            .iter()
            .filter(|resolution| {
                self.edge_exists(
                    EdgeType::Produces,
                    NodeType::Resolution,
                    &resolution.id,
                    NodeType::Rule,
                    rule_id,
                )
            })
            .collect()
    }

    /// Which required producers this rule is missing, in
    /// [`RuleProducer::REQUIRED`] order.
    pub fn missing_rule_producers(&self, rule_id: &StableId) -> Vec<RuleProducer> {
        RuleProducer::REQUIRED
            .into_iter()
            .filter(|producer| match producer {
                RuleProducer::Requirement => self.producing_requirements(rule_id).is_empty(),
                RuleProducer::Resolution => self.producing_resolutions(rule_id).is_empty(),
            })
            .collect()
    }

    /// True when a source reaches this rule through a requirement that
    /// produces it. A sourced requirement elsewhere in the scope says
    /// nothing about this rule.
    pub fn rule_trace_reaches_source(&self, rule_id: &StableId) -> bool {
        self.producing_requirements(rule_id)
            .into_iter()
            .any(|requirement| self.requirement_has_valid_source(requirement))
    }

    pub fn requirement_has_valid_source(&self, requirement: &Requirement) -> bool {
        requirement
            .source_refs
            .iter()
            .any(|reference| self.source_exists(&reference.source_id))
            || self.edges().any(|edge| {
                edge.edge_type == EdgeType::References
                    && edge.from_type == NodeType::Source
                    && self.source_exists(&edge.from_id)
                    && edge.to_type == NodeType::Requirement
                    && edge.to_id == requirement.id
            })
    }

    pub fn source_is_referenced(&self, source_id: &StableId) -> bool {
        self.graph.requirements.iter().any(|requirement| {
            requirement
                .source_refs
                .iter()
                .any(|reference| reference.source_id == *source_id)
        }) || self.graph.requirements.iter().any(|requirement| {
            self.edge_exists(
                EdgeType::References,
                NodeType::Source,
                source_id,
                NodeType::Requirement,
                &requirement.id,
            )
        })
    }
}
