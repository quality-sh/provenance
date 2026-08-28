use provenance_core::{NodeType, StableId};

/// Why a graph record appears on the shaping frontier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GapKind {
    /// A requirement has no domain assignment.
    MissingDomainId,
    /// A requirement has no source refs and no `references` edge.
    MissingSourceRefs,
    /// A resolved requirement has no `resolves` edge pointing at it.
    NoResolvingDecision,
    /// A resolved or decision-backed requirement has no downstream rule.
    NoProducedRules,
    /// No requirement records the rule it refines.
    OrphanRule,
    /// A resolution resolves no requirement.
    OrphanResolution,
    /// A source nothing references.
    UnreferencedSource,
    /// A reference to a record that does not exist in the scope.
    DanglingReference,
    /// A `contradicts` pair has no resolving decision.
    UnresolvedContradictsPair,
    /// A question is still open.
    OpenQuestion,
    /// A topic is still unexplored.
    UnexploredTopic,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct GapItem {
    pub kind: GapKind,
    pub node_type: NodeType,
    pub node_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub related_node_type: Option<NodeType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub related_node_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requirement_id: Option<String>,
    pub reason: String,
}

impl GapItem {
    pub(super) fn new(
        kind: GapKind,
        node_type: NodeType,
        node_id: &StableId,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            node_type,
            node_id: node_id.as_str().to_string(),
            related_node_type: None,
            related_node_id: None,
            requirement_id: (node_type == NodeType::Requirement)
                .then(|| node_id.as_str().to_string()),
            reason: reason.into(),
        }
    }

    pub(super) fn with_related(mut self, node_type: NodeType, node_id: &StableId) -> Self {
        self.related_node_type = Some(node_type);
        self.related_node_id = Some(node_id.as_str().to_string());
        self
    }

    pub(super) fn with_requirement(mut self, requirement_id: &StableId) -> Self {
        self.requirement_id = Some(requirement_id.as_str().to_string());
        self
    }

    pub fn subject(&self) -> String {
        let subject = format!("{} {}", node_type_word(self.node_type), self.node_id);
        match (self.related_node_type, self.related_node_id.as_deref()) {
            (Some(related_type), Some(related_id)) => {
                format!("{subject} -> {} {related_id}", node_type_word(related_type))
            }
            _ => subject,
        }
    }
}

pub const fn node_type_word(node_type: NodeType) -> &'static str {
    match node_type {
        NodeType::Source => "source",
        NodeType::Requirement => "requirement",
        NodeType::Resolution => "resolution",
        NodeType::Rule => "rule",
        NodeType::Topic => "topic",
        NodeType::Question => "question",
        NodeType::Domain => "domain",
        NodeType::Boundary => "boundary",
    }
}
