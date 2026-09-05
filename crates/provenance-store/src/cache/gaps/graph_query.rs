use provenance_core::model::relations::RecordFront;
use provenance_core::{
    Boundary, Domain, NodeType, Question, Requirement, Resolution, Rule, ScopeId, Source, StableId,
    Thread, Topic,
};

pub struct GapGraph<'a> {
    pub scope: &'a ScopeId,
    pub sources: &'a [Source],
    pub requirements: &'a [Requirement],
    pub resolutions: &'a [Resolution],
    pub rules: &'a [Rule],
    pub topics: &'a [Topic],
    pub questions: &'a [Question],
    pub threads: &'a [Thread],
    pub domains: &'a [Domain],
    pub boundaries: &'a [Boundary],
}

impl GapGraph<'_> {
    /// The traversal front over these records.
    pub const fn front(&self) -> RecordFront<'_> {
        RecordFront {
            sources: self.sources,
            requirements: self.requirements,
            resolutions: self.resolutions,
            rules: self.rules,
            topics: self.topics,
            questions: self.questions,
            domains: self.domains,
            boundaries: self.boundaries,
        }
    }
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

    /// The resolutions whose `requirement_ids` name the requirement.
    pub fn resolving_resolutions(&self, requirement_id: &StableId) -> Vec<&'graph Resolution> {
        self.graph
            .resolutions
            .iter()
            .filter(|resolution| resolution.requirement_ids.contains(requirement_id))
            .collect()
    }

    /// The rules a requirement produces: named in `requirement_ids`, or
    /// named in `resolution_ids` by a resolution that resolves it.
    pub fn produced_rules_for_requirement(&self, requirement_id: &StableId) -> Vec<&'graph Rule> {
        let resolving = self.resolving_resolutions(requirement_id);
        self.graph
            .rules
            .iter()
            .filter(|rule| {
                rule.requirement_ids.contains(requirement_id)
                    || rule.resolution_ids.iter().any(|resolution| {
                        resolving
                            .iter()
                            .any(|resolving| resolving.id == *resolution)
                    })
            })
            .collect()
    }

    pub fn produced_rules_for_resolution(&self, resolution_id: &StableId) -> Vec<&'graph Rule> {
        self.graph
            .rules
            .iter()
            .filter(|rule| rule.resolution_ids.contains(resolution_id))
            .collect()
    }

    /// The requirements a rule names. A named requirement that is not in
    /// the scope is a dangling reference, not a producer.
    pub fn producing_requirements(&self, rule_id: &StableId) -> Vec<&'graph Requirement> {
        let Some(rule) = self.graph.rules.iter().find(|rule| rule.id == *rule_id) else {
            return Vec::new();
        };
        self.graph
            .requirements
            .iter()
            .filter(|requirement| rule.requirement_ids.contains(&requirement.id))
            .collect()
    }

    /// The resolutions a rule names, on the same terms as
    /// [`Self::producing_requirements`].
    pub fn producing_resolutions(&self, rule_id: &StableId) -> Vec<&'graph Resolution> {
        let Some(rule) = self.graph.rules.iter().find(|rule| rule.id == *rule_id) else {
            return Vec::new();
        };
        self.graph
            .resolutions
            .iter()
            .filter(|resolution| rule.resolution_ids.contains(&resolution.id))
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
    }

    pub fn source_is_referenced(&self, source_id: &StableId) -> bool {
        self.graph.requirements.iter().any(|requirement| {
            requirement
                .source_refs
                .iter()
                .any(|reference| reference.source_id == *source_id)
        })
    }
}
