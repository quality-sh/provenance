mod relation_tables {
use crate::model::relations::{declared_relations, RelationDecl, RelationFlow};
use crate::model::NodeType;
use provenance_macros::verifies;

use NodeType::{Boundary, Domain, Question, Requirement, Resolution, Rule, Source, Topic};
use RelationFlow::{None, TargetDownstream, TargetUpstream};

/// The seven kinds that declare relations, in node rank order. Domain
/// references nothing.
const OWNERS: [NodeType; 7] = [Source, Requirement, Resolution, Rule, Topic, Question, Boundary];

/// The section C table: (owner, name, target, list, required, flow).
const TABLE: [(NodeType, &str, NodeType, bool, bool, RelationFlow); 18] = [
    (Source, "supersedes", Source, true, false, TargetDownstream),
    (Requirement, "domain_id", Domain, false, false, None),
    (Requirement, "cites", Source, true, false, TargetUpstream),
    (Requirement, "refines", Requirement, false, false, TargetUpstream),
    (Requirement, "depends_on", Requirement, true, false, TargetDownstream),
    (Requirement, "supersedes", Requirement, true, false, TargetDownstream),
    (Requirement, "spawned_by", Resolution, false, false, TargetUpstream),
    (Resolution, "requirement_ids", Requirement, true, true, TargetUpstream),
    (Resolution, "supersedes", Resolution, true, false, TargetDownstream),
    (Rule, "requirement_ids", Requirement, true, true, TargetUpstream),
    (Rule, "resolution_ids", Resolution, true, false, TargetUpstream),
    (Topic, "requirement_id", Requirement, false, true, None),
    (Question, "topic_id", Topic, false, true, None),
    (Question, "requirement_id", Requirement, false, true, None),
    (Question, "resolution_id", Resolution, false, false, None),
    (Question, "contradicts", Requirement, false, false, None),
    (Boundary, "requirement_id", Requirement, false, true, None),
    (Boundary, "cites", Source, false, false, None),
];

fn row_of(decl: &RelationDecl) -> (NodeType, &'static str, NodeType, bool, bool, RelationFlow) {
    (
        decl.owner,
        decl.name,
        decl.target,
        decl.list,
        decl.required,
        decl.flow,
    )
}

#[test]
#[verifies("rule_prov_relation_vocabulary_closed", exhaustion)]
fn every_owner_kind_appears_once_in_the_declared_tables() {
    let tables = declared_relations();
    assert_eq!(tables.len(), OWNERS.len());
    for (table, owner) in tables.iter().zip(OWNERS) {
        assert!(!table.is_empty(), "{owner:?} declares at least one relation");
        assert!(
            table.iter().all(|decl| decl.owner == owner),
            "the table at {owner:?}'s position carries another owner"
        );
    }
    let rows: Vec<_> = tables.iter().flat_map(|table| table.iter().map(row_of)).collect();
    assert_eq!(rows, TABLE);
}

/// `links` on topics and questions carries a per-entry target kind, so it is
/// walked by hand and declared in no table: two declarations, one name.
#[test]
fn the_vocabulary_is_thirteen_names_over_twenty_declarations() {
    let mut names: Vec<&str> = TABLE.iter().map(|row| row.1).collect();
    names.push("links");
    names.sort_unstable();
    names.dedup();
    assert_eq!(TABLE.len() + 2, 20);
    assert_eq!(
        names,
        [
            "cites",
            "contradicts",
            "depends_on",
            "domain_id",
            "links",
            "refines",
            "requirement_id",
            "requirement_ids",
            "resolution_id",
            "resolution_ids",
            "spawned_by",
            "supersedes",
            "topic_id",
        ]
    );
}
}
