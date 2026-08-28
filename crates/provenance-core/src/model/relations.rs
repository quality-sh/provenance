//! The closed relation vocabulary.
//!
//! One `RelationKind` parameterizes every traversal over canonical state:
//! the nine edge rows, the six foreign-key attachments, and the embedded
//! reference collections. The enum admits no wildcard fallback: traversals
//! match exhaustively, so a relationship family without a declared variant
//! cannot traverse at all - a compile error, not a runtime filter. This is
//! the pinned-graph mechanism applied to relations: the variant list is
//! the rule, and adding a variant is how the rule changes.
//!
//! Ideation target references join only after the dangling-target
//! validation prerequisite lands; the enum is closed until then.

use super::graph::{EdgeType, NodeType};

/// Where a relation's second endpoint lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationEndpoint {
    /// A fixed record kind.
    Node(NodeType),
    /// The target type carried by each `ArtifactLink` element.
    LinkTarget,
}

/// How the relation is derived from canonical state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationDerivation {
    /// A row in the global edge shard.
    EdgeRow(EdgeType),
    /// A foreign-key field on the owning record.
    FkField,
    /// An embedded reference collection on the owning record.
    EmbeddedCollection,
}

/// Every relationship family canonical state can express.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationKind {
    // The nine edge types, in `edge_rank` order.
    SourceReferencesRequirement,
    RequirementRefinesIntoRequirement,
    RequirementDependsOnRequirement,
    RequirementContradictsRequirement,
    RequirementSupersedesRequirement,
    RequirementNeedsResolution,
    ResolutionResolvesRequirement,
    ResolutionSpawnsRequirement,
    RequirementProducesRule,
    ResolutionProducesRule,
    // The six foreign-key attachments.
    BoundaryRequiresRequirement,
    TopicExploresRequirement,
    QuestionRefinesTopic,
    QuestionRaisesRequirement,
    QuestionSeeksResolution,
    RequirementBelongsToDomain,
    // The embedded reference collections.
    RequirementCitesSource,
    TopicLinksArtifact,
    QuestionLinksArtifact,
}

impl RelationKind {
    /// The owning endpoint, the target endpoint, and the derivation.
    pub const fn parts(self) -> (NodeType, RelationEndpoint, RelationDerivation) {
        match self {
            Self::SourceReferencesRequirement => (
                NodeType::Source,
                RelationEndpoint::Node(NodeType::Requirement),
                RelationDerivation::EdgeRow(EdgeType::References),
            ),
            Self::RequirementRefinesIntoRequirement => (
                NodeType::Requirement,
                RelationEndpoint::Node(NodeType::Requirement),
                RelationDerivation::EdgeRow(EdgeType::RefinesInto),
            ),
            Self::RequirementDependsOnRequirement => (
                NodeType::Requirement,
                RelationEndpoint::Node(NodeType::Requirement),
                RelationDerivation::EdgeRow(EdgeType::DependsOn),
            ),
            Self::RequirementContradictsRequirement => (
                NodeType::Requirement,
                RelationEndpoint::Node(NodeType::Requirement),
                RelationDerivation::EdgeRow(EdgeType::Contradicts),
            ),
            Self::RequirementSupersedesRequirement => (
                NodeType::Requirement,
                RelationEndpoint::Node(NodeType::Requirement),
                RelationDerivation::EdgeRow(EdgeType::Supersedes),
            ),
            Self::RequirementNeedsResolution => (
                NodeType::Requirement,
                RelationEndpoint::Node(NodeType::Resolution),
                RelationDerivation::EdgeRow(EdgeType::Needs),
            ),
            Self::ResolutionResolvesRequirement => (
                NodeType::Resolution,
                RelationEndpoint::Node(NodeType::Requirement),
                RelationDerivation::EdgeRow(EdgeType::Resolves),
            ),
            Self::ResolutionSpawnsRequirement => (
                NodeType::Resolution,
                RelationEndpoint::Node(NodeType::Requirement),
                RelationDerivation::EdgeRow(EdgeType::Spawns),
            ),
            Self::RequirementProducesRule => (
                NodeType::Requirement,
                RelationEndpoint::Node(NodeType::Rule),
                RelationDerivation::EdgeRow(EdgeType::Produces),
            ),
            Self::ResolutionProducesRule => (
                NodeType::Resolution,
                RelationEndpoint::Node(NodeType::Rule),
                RelationDerivation::EdgeRow(EdgeType::Produces),
            ),
            Self::BoundaryRequiresRequirement => (
                NodeType::Boundary,
                RelationEndpoint::Node(NodeType::Requirement),
                RelationDerivation::FkField,
            ),
            Self::TopicExploresRequirement => (
                NodeType::Topic,
                RelationEndpoint::Node(NodeType::Requirement),
                RelationDerivation::FkField,
            ),
            Self::QuestionRefinesTopic => (
                NodeType::Question,
                RelationEndpoint::Node(NodeType::Topic),
                RelationDerivation::FkField,
            ),
            Self::QuestionRaisesRequirement => (
                NodeType::Question,
                RelationEndpoint::Node(NodeType::Requirement),
                RelationDerivation::FkField,
            ),
            Self::QuestionSeeksResolution => (
                NodeType::Question,
                RelationEndpoint::Node(NodeType::Resolution),
                RelationDerivation::FkField,
            ),
            Self::RequirementBelongsToDomain => (
                NodeType::Requirement,
                RelationEndpoint::Node(NodeType::Domain),
                RelationDerivation::FkField,
            ),
            Self::RequirementCitesSource => (
                NodeType::Requirement,
                RelationEndpoint::Node(NodeType::Source),
                RelationDerivation::EmbeddedCollection,
            ),
            Self::TopicLinksArtifact => (
                NodeType::Topic,
                RelationEndpoint::LinkTarget,
                RelationDerivation::EmbeddedCollection,
            ),
            Self::QuestionLinksArtifact => (
                NodeType::Question,
                RelationEndpoint::LinkTarget,
                RelationDerivation::EmbeddedCollection,
            ),
        }
    }

    /// The edge type behind an edge-row relation, if it is one.
    pub const fn edge_type(self) -> Option<EdgeType> {
        match self.parts().2 {
            RelationDerivation::EdgeRow(edge_type) => Some(edge_type),
            _ => None,
        }
    }
}
