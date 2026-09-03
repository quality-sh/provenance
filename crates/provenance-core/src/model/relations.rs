//! The closed vocabulary of connections between canonical records.
//!
//! Three connection styles exist in the data model: validated edge rows,
//! foreign-key fields, and embedded reference collections. Each declared
//! relation names one of them as its derivation, so every traversal reads
//! the same vocabulary and a connection style without a declared variant
//! cannot traverse at all. The variant list is the rule: traversals match
//! exhaustively, and a wildcard arm is a review failure, not a shortcut.
//!
//! This is a closed parameterization of fixed operations. It carries no
//! predicate language and no composition grammar, and it must not grow one.

mod decl;
mod front;
mod integrity;

pub use decl::{declaration_of, RelationDecl, RelationFlow, RelationOwner};
pub use front::{
    declaration_for, flow_neighbors, incoming_of, link_rows_of, link_target, outgoing_of,
    related_nodes, rows_of, RecordFront, RelatedNode, RelationDirection, RelationEndpoint,
    RelationRow, RelationSource, LINKS,
};
pub use integrity::{
    cycle_in, cycle_refusal, kind_word, missing_required, reaches, required_refusal,
};

use super::graph::{EdgeType, NodeType};
use super::shaping::{Boundary, Question, Topic};
use super::{Requirement, Resolution, Rule, Source};
use provenance_macros::rule;

/// The declaration tables of the seven owner kinds, in node rank order.
///
/// Each table is written by `#[derive(Relations)]` from the record's own
/// fields; this list is the one hand-written concatenation.
#[rule("rule_prov_relation_vocabulary_closed")]
pub const fn declared_relations() -> &'static [&'static [RelationDecl]] {
    &[
        &Source::RELATIONS,
        &Requirement::RELATIONS,
        &Resolution::RELATIONS,
        &Rule::RELATIONS,
        &Topic::RELATIONS,
        &Question::RELATIONS,
        &Boundary::RELATIONS,
    ]
}

/// True when some declaration, or `links`, carries the name.
pub fn is_relation_name(name: &str) -> bool {
    name == LINKS
        || declared_relations()
            .iter()
            .flat_map(|table| table.iter())
            .any(|decl| decl.name == name)
}

/// The edge-era vocabulary, kept until the readers move off it.
pub const fn relation_kinds() -> &'static [RelationKind] {
    &RelationKind::ALL
}

/// How a relation is stored in canonical state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationDerivation {
    /// A validated row in the global edges shard.
    EdgeRow,
    /// A foreign-key field on the from-side record.
    FkField,
    /// A list of references embedded in the from-side record.
    EmbeddedCollection,
}

/// One declared connection between two record kinds.
///
/// The declared direction is from-side to to-side: the side that stores the
/// connection points at the side it names. A traversal may still walk the
/// reverse direction; that is a labeled reverse scan over the same
/// declaration, never a second relation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationKind {
    References,
    RefinesInto,
    DependsOn,
    Contradicts,
    Supersedes,
    Needs,
    Resolves,
    Spawns,
    Produces,
    BoundaryConstrains,
    TopicShapes,
    QuestionBelongsToTopic,
    QuestionRefines,
    QuestionSettledBy,
    RequirementInDomain,
    RequirementCitesSource,
    TopicLinks,
    QuestionLinks,
    SourceSupersededBy,
    ResolutionSupersededBy,
    BoundaryCitesSource,
}

const LINKABLE: &[NodeType] = &[
    NodeType::Source,
    NodeType::Requirement,
    NodeType::Resolution,
    NodeType::Rule,
];

impl RelationKind {
    pub const ALL: [Self; 21] = [
        Self::References,
        Self::RefinesInto,
        Self::DependsOn,
        Self::Contradicts,
        Self::Supersedes,
        Self::Needs,
        Self::Resolves,
        Self::Spawns,
        Self::Produces,
        Self::BoundaryConstrains,
        Self::TopicShapes,
        Self::QuestionBelongsToTopic,
        Self::QuestionRefines,
        Self::QuestionSettledBy,
        Self::RequirementInDomain,
        Self::RequirementCitesSource,
        Self::TopicLinks,
        Self::QuestionLinks,
        Self::SourceSupersededBy,
        Self::ResolutionSupersededBy,
        Self::BoundaryCitesSource,
    ];

    /// The relation's one name on the wire and in labels.
    pub const fn name(self) -> &'static str {
        match self {
            Self::References => "references",
            Self::RefinesInto => "refines_into",
            Self::DependsOn => "depends_on",
            Self::Contradicts => "contradicts",
            Self::Supersedes => "supersedes",
            Self::Needs => "needs",
            Self::Resolves => "resolves",
            Self::Spawns => "spawns",
            Self::Produces => "produces",
            Self::BoundaryConstrains => "boundary_constrains",
            Self::TopicShapes => "topic_shapes",
            Self::QuestionBelongsToTopic => "question_belongs_to_topic",
            Self::QuestionRefines => "question_refines",
            Self::QuestionSettledBy => "question_settled_by",
            Self::RequirementInDomain => "requirement_in_domain",
            Self::RequirementCitesSource => "requirement_cites_source",
            Self::TopicLinks => "topic_links",
            Self::QuestionLinks => "question_links",
            Self::SourceSupersededBy => "source_superseded_by",
            Self::ResolutionSupersededBy => "resolution_superseded_by",
            Self::BoundaryCitesSource => "boundary_cites_source",
        }
    }

    /// How canonical state stores this relation.
    pub const fn derivation(self) -> RelationDerivation {
        match self {
            Self::References
            | Self::RefinesInto
            | Self::DependsOn
            | Self::Contradicts
            | Self::Supersedes
            | Self::Needs
            | Self::Resolves
            | Self::Spawns
            | Self::Produces => RelationDerivation::EdgeRow,
            Self::BoundaryConstrains
            | Self::TopicShapes
            | Self::QuestionBelongsToTopic
            | Self::QuestionRefines
            | Self::QuestionSettledBy
            | Self::RequirementInDomain
            | Self::SourceSupersededBy
            | Self::ResolutionSupersededBy
            | Self::BoundaryCitesSource => RelationDerivation::FkField,
            Self::RequirementCitesSource | Self::TopicLinks | Self::QuestionLinks => {
                RelationDerivation::EmbeddedCollection
            }
        }
    }

    /// The edge type behind an edge-row relation.
    pub const fn edge_type(self) -> Option<EdgeType> {
        match self {
            Self::References => Some(EdgeType::References),
            Self::RefinesInto => Some(EdgeType::RefinesInto),
            Self::DependsOn => Some(EdgeType::DependsOn),
            Self::Contradicts => Some(EdgeType::Contradicts),
            Self::Supersedes => Some(EdgeType::Supersedes),
            Self::Needs => Some(EdgeType::Needs),
            Self::Resolves => Some(EdgeType::Resolves),
            Self::Spawns => Some(EdgeType::Spawns),
            Self::Produces => Some(EdgeType::Produces),
            Self::BoundaryConstrains
            | Self::TopicShapes
            | Self::QuestionBelongsToTopic
            | Self::QuestionRefines
            | Self::QuestionSettledBy
            | Self::RequirementInDomain
            | Self::RequirementCitesSource
            | Self::TopicLinks
            | Self::QuestionLinks
            | Self::SourceSupersededBy
            | Self::ResolutionSupersededBy
            | Self::BoundaryCitesSource => None,
        }
    }

    /// The record kinds a relation may leave from.
    pub const fn from_types(self) -> &'static [NodeType] {
        match self {
            Self::References | Self::SourceSupersededBy => &[NodeType::Source],
            Self::RefinesInto
            | Self::DependsOn
            | Self::Contradicts
            | Self::Supersedes
            | Self::Needs
            | Self::RequirementInDomain
            | Self::RequirementCitesSource => &[NodeType::Requirement],
            Self::Resolves | Self::Spawns | Self::ResolutionSupersededBy => &[NodeType::Resolution],
            Self::Produces => &[NodeType::Requirement, NodeType::Resolution],
            Self::BoundaryConstrains | Self::BoundaryCitesSource => &[NodeType::Boundary],
            Self::TopicShapes | Self::TopicLinks => &[NodeType::Topic],
            Self::QuestionBelongsToTopic
            | Self::QuestionRefines
            | Self::QuestionSettledBy
            | Self::QuestionLinks => &[NodeType::Question],
        }
    }

    /// The relation that records the same fact through another connection
    /// style, when one exists.
    ///
    /// The standard citation write path stores one fact twice: the embedded
    /// `source_refs` entry and the `References` edge row. The duality is
    /// declared here so a traversal can present the fact once; it is
    /// symmetric, and the vocabulary-earlier relation speaks for the pair.
    pub const fn same_fact_as(self) -> Option<Self> {
        match self {
            Self::References => Some(Self::RequirementCitesSource),
            Self::RequirementCitesSource => Some(Self::References),
            Self::RefinesInto
            | Self::DependsOn
            | Self::Contradicts
            | Self::Supersedes
            | Self::Needs
            | Self::Resolves
            | Self::Spawns
            | Self::Produces
            | Self::BoundaryConstrains
            | Self::TopicShapes
            | Self::QuestionBelongsToTopic
            | Self::QuestionRefines
            | Self::QuestionSettledBy
            | Self::RequirementInDomain
            | Self::TopicLinks
            | Self::QuestionLinks
            | Self::SourceSupersededBy
            | Self::ResolutionSupersededBy
            | Self::BoundaryCitesSource => None,
        }
    }

    /// The record kinds a relation may point at.
    pub const fn to_types(self) -> &'static [NodeType] {
        match self {
            Self::References
            | Self::RefinesInto
            | Self::DependsOn
            | Self::Contradicts
            | Self::Supersedes
            | Self::Resolves
            | Self::Spawns
            | Self::BoundaryConstrains
            | Self::TopicShapes
            | Self::QuestionRefines => &[NodeType::Requirement],
            Self::Needs | Self::QuestionSettledBy | Self::ResolutionSupersededBy => {
                &[NodeType::Resolution]
            }
            Self::Produces => &[NodeType::Rule],
            Self::QuestionBelongsToTopic => &[NodeType::Topic],
            Self::RequirementInDomain => &[NodeType::Domain],
            Self::RequirementCitesSource | Self::SourceSupersededBy | Self::BoundaryCitesSource => {
                &[NodeType::Source]
            }
            Self::TopicLinks | Self::QuestionLinks => LINKABLE,
        }
    }
}
