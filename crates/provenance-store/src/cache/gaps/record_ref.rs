//! One located graph record, labelled with the kind that located it.

use provenance_core::{
    Boundary, Domain, NodeType, Question, Requirement, Resolution, Rule, Source, StableId, Topic,
};

/// One located graph record, labelled with the kind that located it.
///
/// Ids are kind-qualified: a Requirement and a Resolution sharing a
/// stable id are two records, and a lookup under one kind never returns
/// the other.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RecordRef<'graph> {
    Source(&'graph Source),
    Requirement(&'graph Requirement),
    Resolution(&'graph Resolution),
    Rule(&'graph Rule),
    Topic(&'graph Topic),
    Question(&'graph Question),
    Domain(&'graph Domain),
    Boundary(&'graph Boundary),
}

impl RecordRef<'_> {
    /// The stable id of the record behind the reference.
    pub const fn id(&self) -> &StableId {
        match self {
            RecordRef::Source(record) => &record.id,
            RecordRef::Requirement(record) => &record.id,
            RecordRef::Resolution(record) => &record.id,
            RecordRef::Rule(record) => &record.id,
            RecordRef::Topic(record) => &record.id,
            RecordRef::Question(record) => &record.id,
            RecordRef::Domain(record) => &record.id,
            RecordRef::Boundary(record) => &record.id,
        }
    }

    /// The node kind the record was sought under.
    pub const fn node_type(&self) -> NodeType {
        match self {
            RecordRef::Source(_) => NodeType::Source,
            RecordRef::Requirement(_) => NodeType::Requirement,
            RecordRef::Resolution(_) => NodeType::Resolution,
            RecordRef::Rule(_) => NodeType::Rule,
            RecordRef::Topic(_) => NodeType::Topic,
            RecordRef::Question(_) => NodeType::Question,
            RecordRef::Domain(_) => NodeType::Domain,
            RecordRef::Boundary(_) => NodeType::Boundary,
        }
    }
}
