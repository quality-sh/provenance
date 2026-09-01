//! One traversal home over the declared relations.
//!
//! The core walks the closed vocabulary through a provider seam. A front
//! answers one question — which endpoints does this relation connect to
//! this record, in this direction — and the executor owns iteration order
//! and direction legality. `RecordFront` is the in-memory front over the
//! record vectors the gap policy already builds; the indexed `SqlFront`
//! joins in the W3 re-back through the same seam.

use super::{RelationDerivation, RelationKind};
use crate::model::graph::{Edge, EdgeType, NodeType};
use crate::model::ids::StableId;
use crate::model::services::Domain;
use crate::model::shaping::{ArtifactLink, ArtifactLinkTargetType, Boundary, Question, Topic};
use crate::model::{Requirement, Resolution, Rule, Source};

/// Which way a traversal reads a declared relation.
///
/// `Out` follows the declared direction: the side that stores the
/// connection points at the side it names. `In` is the labeled reverse
/// scan over the same declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationDirection {
    Out,
    In,
}

/// One record a relation connects to, as stored.
///
/// The front reports the declared endpoint and never checks that the
/// record behind it exists; dangling references are the gap policy's
/// subject, not the traversal's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationEndpoint {
    pub node_type: NodeType,
    pub id: StableId,
}

/// One reached record with the relation and direction that reached it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelatedNode {
    pub relation: RelationKind,
    pub direction: RelationDirection,
    pub endpoint: RelationEndpoint,
}

/// The provider seam the traversal core executes over.
pub trait RelationSource {
    /// The endpoints `relation` connects to the named record, walking
    /// `direction`. A record kind outside the relation's declared endpoint
    /// set answers empty.
    ///
    /// Ordering contract: every implementation answers in node rank then
    /// canonical id order, so two fronts over the same records answer the
    /// same bytes.
    fn related(
        &self,
        relation: RelationKind,
        node_type: NodeType,
        id: &StableId,
        direction: RelationDirection,
    ) -> Vec<RelationEndpoint>;
}

/// Walks every declared relation around one record, in vocabulary order,
/// out before in, and labels each reached record with what reached it.
pub fn related_nodes<S: RelationSource>(
    source: &S,
    node_type: NodeType,
    id: &StableId,
) -> Vec<RelatedNode> {
    let mut reached = Vec::new();
    for relation in RelationKind::ALL {
        for direction in [RelationDirection::Out, RelationDirection::In] {
            let legal = match direction {
                RelationDirection::Out => relation.from_types().contains(&node_type),
                RelationDirection::In => relation.to_types().contains(&node_type),
            };
            if !legal {
                continue;
            }
            for endpoint in source.related(relation, node_type, id, direction) {
                reached.push(RelatedNode {
                    relation,
                    direction,
                    endpoint,
                });
            }
        }
    }
    drop_duality_echoes(reached)
}

/// Drops the later half of a declared same-fact duality, so one fact is
/// presented once. The vocabulary-earlier relation speaks for the pair.
fn drop_duality_echoes(reached: Vec<RelatedNode>) -> Vec<RelatedNode> {
    let position = |kind: RelationKind| {
        RelationKind::ALL
            .iter()
            .position(|candidate| *candidate == kind)
            .expect("every kind is in ALL")
    };
    let spoken_for: Vec<(RelationKind, StableId)> = reached
        .iter()
        .map(|node| (node.relation, node.endpoint.id.clone()))
        .collect();
    reached
        .into_iter()
        .filter(|node| {
            node.relation.same_fact_as().is_none_or(|partner| {
                position(partner) > position(node.relation)
                    || !spoken_for
                        .iter()
                        .any(|(kind, id)| *kind == partner && *id == node.endpoint.id)
            })
        })
        .collect()
}

/// The in-memory front: record vectors, the shape the gap policy builds.
pub struct RecordFront<'a> {
    pub sources: &'a [Source],
    pub requirements: &'a [Requirement],
    pub resolutions: &'a [Resolution],
    pub rules: &'a [Rule],
    pub topics: &'a [Topic],
    pub questions: &'a [Question],
    pub domains: &'a [Domain],
    pub boundaries: &'a [Boundary],
    pub edges: &'a [Edge],
}

impl RelationSource for RecordFront<'_> {
    fn related(
        &self,
        relation: RelationKind,
        node_type: NodeType,
        id: &StableId,
        direction: RelationDirection,
    ) -> Vec<RelationEndpoint> {
        let legal = match direction {
            RelationDirection::Out => relation.from_types().contains(&node_type),
            RelationDirection::In => relation.to_types().contains(&node_type),
        };
        if !legal {
            return Vec::new();
        }
        match relation.derivation() {
            RelationDerivation::EdgeRow => {
                let Some(edge_type) = relation.edge_type() else {
                    return Vec::new();
                };
                in_contract_order(
                    self.edge_endpoints(relation, edge_type, node_type, id, direction),
                )
            }
            RelationDerivation::FkField => {
                in_contract_order(self.fk_related(relation, id, direction))
            }
            RelationDerivation::EmbeddedCollection => {
                in_contract_order(self.embedded_related(relation, node_type, id, direction))
            }
        }
    }
}

impl RecordFront<'_> {
    /// The foreign-key half of the vocabulary. Edge and embedded kinds
    /// answer empty here; their derivation routes them elsewhere.
    fn fk_related(
        &self,
        relation: RelationKind,
        id: &StableId,
        direction: RelationDirection,
    ) -> Vec<RelationEndpoint> {
        match relation {
            RelationKind::References
            | RelationKind::RefinesInto
            | RelationKind::DependsOn
            | RelationKind::Contradicts
            | RelationKind::Supersedes
            | RelationKind::Needs
            | RelationKind::Resolves
            | RelationKind::Spawns
            | RelationKind::Produces
            | RelationKind::RequirementCitesSource
            | RelationKind::TopicLinks
            | RelationKind::QuestionLinks => Vec::new(),
            RelationKind::BoundaryConstrains => fk(
                self.boundaries,
                |record| (&record.id, Some(&record.requirement_id)),
                NodeType::Boundary,
                NodeType::Requirement,
                id,
                direction,
            ),
            RelationKind::TopicShapes => fk(
                self.topics,
                |record| (&record.id, Some(&record.requirement_id)),
                NodeType::Topic,
                NodeType::Requirement,
                id,
                direction,
            ),
            RelationKind::QuestionBelongsToTopic => fk(
                self.questions,
                |record| (&record.id, Some(&record.topic_id)),
                NodeType::Question,
                NodeType::Topic,
                id,
                direction,
            ),
            RelationKind::QuestionRefines => fk(
                self.questions,
                |record| (&record.id, Some(&record.requirement_id)),
                NodeType::Question,
                NodeType::Requirement,
                id,
                direction,
            ),
            RelationKind::QuestionSettledBy => fk(
                self.questions,
                |record| (&record.id, record.resolution_id.as_ref()),
                NodeType::Question,
                NodeType::Resolution,
                id,
                direction,
            ),
            RelationKind::RequirementInDomain => fk(
                self.requirements,
                |record| (&record.id, record.domain_id.as_ref()),
                NodeType::Requirement,
                NodeType::Domain,
                id,
                direction,
            ),
            RelationKind::SourceSupersededBy => fk(
                self.sources,
                |record| (&record.id, record.superseded_by.as_ref()),
                NodeType::Source,
                NodeType::Source,
                id,
                direction,
            ),
            RelationKind::ResolutionSupersededBy => fk(
                self.resolutions,
                |record| (&record.id, record.superseded_by.as_ref()),
                NodeType::Resolution,
                NodeType::Resolution,
                id,
                direction,
            ),
            RelationKind::BoundaryCitesSource => fk(
                self.boundaries,
                |record| {
                    (
                        &record.id,
                        record.source_ref.as_ref().map(|cite| &cite.source_id),
                    )
                },
                NodeType::Boundary,
                NodeType::Source,
                id,
                direction,
            ),
        }
    }

    /// The embedded-collection half of the vocabulary. Edge and
    /// foreign-key kinds answer empty here; their derivation routes them
    /// elsewhere.
    fn embedded_related(
        &self,
        relation: RelationKind,
        node_type: NodeType,
        id: &StableId,
        direction: RelationDirection,
    ) -> Vec<RelationEndpoint> {
        match relation {
            RelationKind::References
            | RelationKind::RefinesInto
            | RelationKind::DependsOn
            | RelationKind::Contradicts
            | RelationKind::Supersedes
            | RelationKind::Needs
            | RelationKind::Resolves
            | RelationKind::Spawns
            | RelationKind::Produces
            | RelationKind::BoundaryConstrains
            | RelationKind::TopicShapes
            | RelationKind::QuestionBelongsToTopic
            | RelationKind::QuestionRefines
            | RelationKind::QuestionSettledBy
            | RelationKind::RequirementInDomain
            | RelationKind::SourceSupersededBy
            | RelationKind::ResolutionSupersededBy
            | RelationKind::BoundaryCitesSource => Vec::new(),
            RelationKind::RequirementCitesSource => embedded(
                self.requirements,
                |record| (&record.id, source_ref_endpoints(record)),
                NodeType::Requirement,
                node_type,
                id,
                direction,
            ),
            RelationKind::TopicLinks => embedded(
                self.topics,
                |record| (&record.id, link_endpoints(&record.links)),
                NodeType::Topic,
                node_type,
                id,
                direction,
            ),
            RelationKind::QuestionLinks => embedded(
                self.questions,
                |record| (&record.id, link_endpoints(&record.links)),
                NodeType::Question,
                node_type,
                id,
                direction,
            ),
        }
    }
}

/// Sorts endpoints into the contract order: node rank, then canonical id.
fn in_contract_order(mut endpoints: Vec<RelationEndpoint>) -> Vec<RelationEndpoint> {
    endpoints.sort_by(|left, right| {
        left.node_type
            .rank()
            .cmp(&right.node_type.rank())
            .then_with(|| left.id.as_str().cmp(right.id.as_str()))
    });
    endpoints
}

impl RecordFront<'_> {
    fn edge_endpoints(
        &self,
        relation: RelationKind,
        edge_type: EdgeType,
        node_type: NodeType,
        id: &StableId,
        direction: RelationDirection,
    ) -> Vec<RelationEndpoint> {
        // A stored row whose endpoint kinds fall outside the relation's
        // declared sets does not traverse: presenting an illegal endpoint
        // under a relation's name would launder corrupt data. The check
        // surface owns reporting such rows.
        self.edges
            .iter()
            .filter(|edge| edge.edge_type == edge_type)
            .filter(|edge| {
                relation.from_types().contains(&edge.from_type)
                    && relation.to_types().contains(&edge.to_type)
            })
            .filter_map(|edge| match direction {
                RelationDirection::Out => (edge.from_type == node_type && edge.from_id == *id)
                    .then(|| RelationEndpoint {
                        node_type: edge.to_type,
                        id: edge.to_id.clone(),
                    }),
                RelationDirection::In => {
                    (edge.to_type == node_type && edge.to_id == *id).then(|| RelationEndpoint {
                        node_type: edge.from_type,
                        id: edge.from_id.clone(),
                    })
                }
            })
            .collect()
    }
}

/// Walks one foreign-key field: out reads the field of the named record,
/// in scans for records whose field names the record.
fn fk<T>(
    records: &[T],
    field: impl Fn(&T) -> (&StableId, Option<&StableId>),
    from_type: NodeType,
    to_type: NodeType,
    id: &StableId,
    direction: RelationDirection,
) -> Vec<RelationEndpoint> {
    match direction {
        RelationDirection::Out => records
            .iter()
            .filter_map(|record| {
                let (record_id, target) = field(record);
                (record_id == id).then_some(target).flatten()
            })
            .map(|target| RelationEndpoint {
                node_type: to_type,
                id: target.clone(),
            })
            .collect(),
        RelationDirection::In => records
            .iter()
            .filter_map(|record| {
                let (record_id, target) = field(record);
                (target == Some(id)).then(|| RelationEndpoint {
                    node_type: from_type,
                    id: record_id.clone(),
                })
            })
            .collect(),
    }
}

/// Walks one embedded reference collection: out lists the named record's
/// references, in scans for records whose collection names the record.
fn embedded<T>(
    records: &[T],
    collection: impl Fn(&T) -> (&StableId, Vec<(NodeType, StableId)>),
    from_type: NodeType,
    queried_type: NodeType,
    id: &StableId,
    direction: RelationDirection,
) -> Vec<RelationEndpoint> {
    match direction {
        RelationDirection::Out => records
            .iter()
            .filter_map(|record| {
                let (record_id, targets) = collection(record);
                (record_id == id).then_some(targets)
            })
            .flatten()
            .map(|(node_type, target)| RelationEndpoint {
                node_type,
                id: target,
            })
            .collect(),
        RelationDirection::In => records
            .iter()
            .filter_map(|record| {
                let (record_id, targets) = collection(record);
                targets
                    .iter()
                    .any(|(target_type, target)| target == id && *target_type == queried_type)
                    .then(|| RelationEndpoint {
                        node_type: from_type,
                        id: record_id.clone(),
                    })
            })
            .collect(),
    }
}

/// The sources a requirement cites through its embedded references.
fn source_ref_endpoints(record: &Requirement) -> Vec<(NodeType, StableId)> {
    record
        .source_refs
        .iter()
        .map(|reference| (NodeType::Source, reference.source_id.clone()))
        .collect()
}

/// The node kinds artifact links name, in the links' own vocabulary.
fn link_endpoints(links: &[ArtifactLink]) -> Vec<(NodeType, StableId)> {
    links
        .iter()
        .map(|link| {
            let node_type = match link.target_type {
                ArtifactLinkTargetType::Source => NodeType::Source,
                ArtifactLinkTargetType::Requirement => NodeType::Requirement,
                ArtifactLinkTargetType::Resolution => NodeType::Resolution,
                ArtifactLinkTargetType::Rule => NodeType::Rule,
            };
            (node_type, link.target_id.clone())
        })
        .collect()
}
