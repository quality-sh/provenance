//! One traversal home over the declared relations.
//!
//! A front answers which records one record's declared relations connect
//! it to, out of its own fields and in from the fields of other records.
//! `RecordFront` is the in-memory front over record vectors; the served
//! `relations` table answers the same rows for the projection.

use super::decl::{declaration_of, RelationDecl, RelationFlow, RelationOwner};
use super::declared_relations;
use crate::model::graph::NodeType;
use crate::model::ids::StableId;
use crate::model::services::Domain;
use crate::model::shaping::{ArtifactLink, ArtifactLinkTargetType, Boundary, Question, Topic};
use crate::model::{Requirement, Resolution, Rule, Source};

/// The relation name of the artifact links topics and questions carry.
pub const LINKS: &str = "links";

/// Which way a traversal reads a declared relation.
///
/// `Out` reads the field of the named record: the record is the owner.
/// `In` scans for records whose field names the record: it is the target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationDirection {
    Out,
    In,
}

/// One record a relation connects to, as stored. The front never checks
/// that the record behind it exists; dangling references are the gap
/// policy's subject, not the traversal's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationEndpoint {
    pub node_type: NodeType,
    pub id: StableId,
}

/// One reached record with the relation and direction that reached it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelatedNode {
    pub relation: &'static str,
    pub direction: RelationDirection,
    pub endpoint: RelationEndpoint,
}

/// One stored relation as a row: the owner, the field, the target.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RelationRow {
    pub owner_type: NodeType,
    pub owner_id: StableId,
    pub relation: String,
    pub target_type: NodeType,
    pub target_id: StableId,
}

/// The provider seam the traversal core executes over.
pub trait RelationSource {
    /// The relations one record holds: (name, target), field by field.
    fn outgoing(&self, node_type: NodeType, id: &StableId)
        -> Vec<(&'static str, RelationEndpoint)>;

    /// The relations that name one record: (name, owner).
    fn incoming(&self, node_type: NodeType, id: &StableId)
        -> Vec<(&'static str, RelationEndpoint)>;
}

/// The declaration behind one relation name on one owner kind. `links` has
/// no declaration: its target kind sits on each entry.
pub fn declaration_for(owner: NodeType, name: &str) -> Option<&'static RelationDecl> {
    declared_relations()
        .iter()
        .flat_map(|table| table.iter())
        .find(|decl| decl.owner == owner && decl.name == name)
}

/// The position of a relation in the declaration order, kinds in node
/// rank, `links` after every table.
fn declaration_index(owner: NodeType, name: &str) -> usize {
    declared_relations()
        .iter()
        .flat_map(|table| table.iter())
        .position(|decl| decl.owner == owner && decl.name == name)
        .unwrap_or(usize::MAX)
}

/// Walks every declared relation around one record, both ways, and labels
/// each reached record with what reached it. Order: node rank, id,
/// declaration order, out before in.
pub fn related_nodes<S: RelationSource>(
    source: &S,
    node_type: NodeType,
    id: &StableId,
) -> Vec<RelatedNode> {
    let mut reached: Vec<(usize, RelatedNode)> = source
        .outgoing(node_type, id)
        .into_iter()
        .map(|(relation, endpoint)| {
            (
                declaration_index(node_type, relation),
                RelatedNode {
                    relation,
                    direction: RelationDirection::Out,
                    endpoint,
                },
            )
        })
        .chain(
            source
                .incoming(node_type, id)
                .into_iter()
                .map(|(relation, endpoint)| {
                    (
                        declaration_index(endpoint.node_type, relation),
                        RelatedNode {
                            relation,
                            direction: RelationDirection::In,
                            endpoint,
                        },
                    )
                }),
        )
        .collect();
    reached.sort_by(|(left_index, left), (right_index, right)| {
        left.endpoint
            .node_type
            .rank()
            .cmp(&right.endpoint.node_type.rank())
            .then_with(|| left.endpoint.id.as_str().cmp(right.endpoint.id.as_str()))
            .then_with(|| left_index.cmp(right_index))
            .then_with(|| direction_rank(left.direction).cmp(&direction_rank(right.direction)))
    });
    reached.into_iter().map(|(_, node)| node).collect()
}

const fn direction_rank(direction: RelationDirection) -> u8 {
    match direction {
        RelationDirection::Out => 0,
        RelationDirection::In => 1,
    }
}

/// The records one hop downstream or upstream of a record.
///
/// Each step derives from the declaration's flow: downstream is out over
/// `target_downstream` and in over `target_upstream`; upstream is the
/// mirror. `none` relations and `links` are never followed.
pub fn flow_neighbors<S: RelationSource>(
    source: &S,
    node_type: NodeType,
    id: &StableId,
    downstream: bool,
) -> Vec<RelatedNode> {
    related_nodes(source, node_type, id)
        .into_iter()
        .filter(|node| {
            let owner = match node.direction {
                RelationDirection::Out => node_type,
                RelationDirection::In => node.endpoint.node_type,
            };
            let Some(decl) = declaration_for(owner, node.relation) else {
                return false;
            };
            match (decl.flow, node.direction) {
                (RelationFlow::TargetDownstream, RelationDirection::Out)
                | (RelationFlow::TargetUpstream, RelationDirection::In) => downstream,
                (RelationFlow::TargetUpstream, RelationDirection::Out)
                | (RelationFlow::TargetDownstream, RelationDirection::In) => !downstream,
                (RelationFlow::None, _) => false,
            }
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
}

impl RelationSource for RecordFront<'_> {
    fn outgoing(
        &self,
        node_type: NodeType,
        id: &StableId,
    ) -> Vec<(&'static str, RelationEndpoint)> {
        match node_type {
            NodeType::Source => outgoing_of(self.sources, id),
            NodeType::Requirement => outgoing_of(self.requirements, id),
            NodeType::Resolution => outgoing_of(self.resolutions, id),
            NodeType::Rule => outgoing_of(self.rules, id),
            NodeType::Topic => {
                let mut rows = outgoing_of(self.topics, id);
                if let Some(topic) = self.topics.iter().find(|topic| topic.id == *id) {
                    rows.extend(link_rows(&topic.links));
                }
                rows
            }
            NodeType::Question => {
                let mut rows = outgoing_of(self.questions, id);
                if let Some(question) = self.questions.iter().find(|question| question.id == *id) {
                    rows.extend(link_rows(&question.links));
                }
                rows
            }
            NodeType::Domain => Vec::new(),
            NodeType::Boundary => outgoing_of(self.boundaries, id),
        }
    }

    fn incoming(
        &self,
        node_type: NodeType,
        id: &StableId,
    ) -> Vec<(&'static str, RelationEndpoint)> {
        let mut rows = incoming_of(self.sources, node_type, id);
        rows.extend(incoming_of(self.requirements, node_type, id));
        rows.extend(incoming_of(self.resolutions, node_type, id));
        rows.extend(incoming_of(self.rules, node_type, id));
        rows.extend(incoming_of(self.topics, node_type, id));
        rows.extend(incoming_of(self.questions, node_type, id));
        rows.extend(incoming_of(self.boundaries, node_type, id));
        for topic in self.topics {
            if links_name(&topic.links, node_type, id) {
                rows.push((LINKS, endpoint(NodeType::Topic, &topic.id)));
            }
        }
        for question in self.questions {
            if links_name(&question.links, node_type, id) {
                rows.push((LINKS, endpoint(NodeType::Question, &question.id)));
            }
        }
        rows
    }
}

fn endpoint(node_type: NodeType, id: &StableId) -> RelationEndpoint {
    RelationEndpoint {
        node_type,
        id: id.clone(),
    }
}

/// The declared references of one record, each with its target kind.
pub fn outgoing_of<T: RelationOwner>(
    records: &[T],
    id: &StableId,
) -> Vec<(&'static str, RelationEndpoint)> {
    let Some(record) = records.iter().find(|record| record.id() == id) else {
        return Vec::new();
    };
    record
        .references()
        .into_iter()
        .map(|(name, target)| {
            let decl =
                declaration_of(T::relations(), name).expect("references name declared fields");
            (name, endpoint(decl.target, target))
        })
        .collect()
}

/// The records of one kind whose declared fields name the target.
pub fn incoming_of<T: RelationOwner>(
    records: &[T],
    target_type: NodeType,
    target: &StableId,
) -> Vec<(&'static str, RelationEndpoint)> {
    let names: Vec<&'static str> = T::relations()
        .iter()
        .filter(|decl| decl.target == target_type)
        .map(|decl| decl.name)
        .collect();
    if names.is_empty() {
        return Vec::new();
    }
    let mut rows = Vec::new();
    for record in records {
        for (name, id) in record.references() {
            if id == target && names.contains(&name) {
                rows.push((name, endpoint(T::OWNER, record.id())));
            }
        }
    }
    rows
}

pub const fn link_target(link: &ArtifactLink) -> NodeType {
    match link.target_type {
        ArtifactLinkTargetType::Source => NodeType::Source,
        ArtifactLinkTargetType::Requirement => NodeType::Requirement,
        ArtifactLinkTargetType::Resolution => NodeType::Resolution,
        ArtifactLinkTargetType::Rule => NodeType::Rule,
    }
}

fn link_rows(links: &[ArtifactLink]) -> Vec<(&'static str, RelationEndpoint)> {
    links
        .iter()
        .map(|link| (LINKS, endpoint(link_target(link), &link.target_id)))
        .collect()
}

fn links_name(links: &[ArtifactLink], node_type: NodeType, id: &StableId) -> bool {
    links
        .iter()
        .any(|link| link_target(link) == node_type && link.target_id == *id)
}

/// Every relation row one owner kind holds, in record then field order.
pub fn rows_of<T: RelationOwner>(records: &[T]) -> Vec<RelationRow> {
    let mut rows = Vec::new();
    for record in records {
        for (name, target) in record.references() {
            let decl =
                declaration_of(T::relations(), name).expect("references name declared fields");
            rows.push(RelationRow {
                owner_type: T::OWNER,
                owner_id: record.id().clone(),
                relation: name.to_string(),
                target_type: decl.target,
                target_id: target.clone(),
            });
        }
    }
    rows
}

/// The `links` rows of one topic or question.
pub fn link_rows_of(owner: NodeType, id: &StableId, links: &[ArtifactLink]) -> Vec<RelationRow> {
    links
        .iter()
        .map(|link| RelationRow {
            owner_type: owner,
            owner_id: id.clone(),
            relation: LINKS.to_string(),
            target_type: link_target(link),
            target_id: link.target_id.clone(),
        })
        .collect()
}
