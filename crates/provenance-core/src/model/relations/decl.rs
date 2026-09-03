//! One declaration per record kind: which fields reference other records.
//!
//! `#[derive(Relations)]` writes the table from the field attributes, so the
//! struct and its declaration cannot drift. Everything that reads or checks a
//! reference reaches the table through [`RelationOwner`].

use crate::model::graph::NodeType;
use crate::model::ids::StableId;

/// Which end of a relation sits upstream in the graph.
///
/// `None` relations are never followed by impact or traceability; trace and
/// neighbors still follow them as the requested direction admits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationFlow {
    TargetUpstream,
    TargetDownstream,
    None,
}

/// One reference field on one record kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RelationDecl {
    pub owner: NodeType,
    pub name: &'static str,
    pub target: NodeType,
    pub list: bool,
    pub required: bool,
    pub flow: RelationFlow,
}

impl RelationDecl {
    /// True when `kind` sits downstream of the owner along this relation.
    pub const fn target_is_downstream(&self) -> bool {
        matches!(self.flow, RelationFlow::TargetDownstream)
    }

    /// True when `kind` sits upstream of the owner along this relation.
    pub const fn target_is_upstream(&self) -> bool {
        matches!(self.flow, RelationFlow::TargetUpstream)
    }
}

/// A record kind that declares its reference fields.
pub trait RelationOwner {
    const OWNER: NodeType;

    fn relations() -> &'static [RelationDecl];

    fn id(&self) -> &StableId;

    /// Every reference the record holds, as (relation name, target id), in
    /// declaration order and then field order.
    fn references(&self) -> Vec<(&'static str, &StableId)>;
}

/// The declaration a named relation of one owner kind, if it exists.
pub fn declaration_of(table: &'static [RelationDecl], name: &str) -> Option<&'static RelationDecl> {
    table.iter().find(|row| row.name == name)
}
