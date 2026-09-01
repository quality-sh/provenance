mod relation_traversal {
use crate::model::relations::{
    related_nodes, RecordFront, RelationDirection, RelationKind, RelationSource,
};
use crate::model::{
    ArtifactLink, ArtifactLinkTargetType, Boundary, Domain, Edge, EdgeType, NodeType, Requirement,
    RequirementStatus, Rule, RuleSeverity, RuleStatus, ScopeId, SchemaVersion, Source,
    SourceReference, SourceType, StableId, Topic, TopicStatus,
};

fn scope() -> ScopeId {
    ScopeId::new("default").unwrap()
}

fn sid(value: &str) -> StableId {
    StableId::new(value).unwrap()
}

fn source(id: &str) -> Source {
    Source {
        schema_version: SchemaVersion(1),
        scope_id: scope(),
        id: sid(id),
        declared_by: None,
        declaration_address: None,
        retired: false,
        name: id.to_string(),
        source_type: SourceType::Policy,
        url: None,
        reference: None,
        commit_pin: None,
        effective_date: None,
        review_date: None,
        superseded_by: None,
        origin_thread: None,
        origin_message: None,
    }
}

fn requirement(id: &str, domain_id: Option<&str>, cites: &[&str]) -> Requirement {
    Requirement {
        schema_version: SchemaVersion(1),
        scope_id: scope(),
        id: sid(id),
        declared_by: None,
        declaration_address: None,
        retired: false,
        statement: format!("{id} statement"),
        description: None,
        fog: None,
        status: RequirementStatus::Active,
        domain_id: domain_id.map(sid),
        source_refs: cites
            .iter()
            .map(|source_id| SourceReference {
                source_id: sid(source_id),
                clause: None,
            })
            .collect(),
        origin_thread: None,
        origin_message: None,
    }
}

fn domain(id: &str) -> Domain {
    Domain {
        schema_version: SchemaVersion(1),
        scope_id: scope(),
        id: sid(id),
        name: id.to_string(),
        description: None,
        color: None,
    }
}

fn boundary(id: &str, requirement_id: &str) -> Boundary {
    Boundary {
        schema_version: SchemaVersion(1),
        scope_id: scope(),
        id: sid(id),
        requirement_id: sid(requirement_id),
        statement: format!("{id} statement"),
        source_ref: None,
    }
}

fn topic(id: &str, requirement_id: &str, links: &[(&str, ArtifactLinkTargetType)]) -> Topic {
    Topic {
        schema_version: SchemaVersion(1),
        scope_id: scope(),
        id: sid(id),
        requirement_id: sid(requirement_id),
        title: id.to_string(),
        status: TopicStatus::Open,
        claimed_by: None,
        claimed_at: None,
        links: links
            .iter()
            .map(|(target_id, target_type)| ArtifactLink {
                target_type: *target_type,
                target_id: sid(target_id),
            })
            .collect(),
    }
}

fn rule(id: &str) -> Rule {
    Rule {
        schema_version: SchemaVersion(1),
        scope_id: scope(),
        id: sid(id),
        declared_by: None,
        declaration_address: None,
        retired: false,
        name: None,
        description: None,
        statement: format!("{id} statement"),
        status: RuleStatus::Active,
        severity: RuleSeverity::High,
        source_document: None,
        source_section: None,
        origin_thread: None,
        origin_message: None,
    }
}

fn edge(id: &str, edge_type: EdgeType, from: (NodeType, &str), to: (NodeType, &str)) -> Edge {
    Edge {
        schema_version: SchemaVersion(1),
        scope_id: scope(),
        id: sid(id),
        edge_type,
        from_type: from.0,
        from_id: sid(from.1),
        to_type: to.0,
        to_id: sid(to.1),
        label: None,
    }
}

struct Fixture {
    sources: Vec<Source>,
    requirements: Vec<Requirement>,
    domains: Vec<Domain>,
    boundaries: Vec<Boundary>,
    topics: Vec<Topic>,
    rules: Vec<Rule>,
    edges: Vec<Edge>,
}

fn fixture() -> Fixture {
    Fixture {
        sources: vec![source("source_award")],
        requirements: vec![requirement(
            "req_overtime",
            Some("domain_payroll"),
            &["source_award"],
        )],
        domains: vec![domain("domain_payroll")],
        boundaries: vec![boundary("boundary_no_backpay", "req_overtime")],
        topics: vec![topic(
            "topic_rates",
            "req_overtime",
            &[("rule_pay", ArtifactLinkTargetType::Rule)],
        )],
        rules: vec![rule("rule_pay")],
        edges: vec![edge(
            "edge_cite",
            EdgeType::References,
            (NodeType::Source, "source_award"),
            (NodeType::Requirement, "req_overtime"),
        )],
    }
}

fn front(records: &Fixture) -> RecordFront<'_> {
    RecordFront {
        sources: &records.sources,
        requirements: &records.requirements,
        resolutions: &[],
        rules: &records.rules,
        topics: &records.topics,
        questions: &[],
        domains: &records.domains,
        boundaries: &records.boundaries,
        edges: &records.edges,
    }
}

fn related(
    records: &Fixture,
    relation: RelationKind,
    node_type: NodeType,
    id: &str,
    direction: RelationDirection,
) -> Vec<(NodeType, String)> {
    front(records)
        .related(relation, node_type, &sid(id), direction)
        .into_iter()
        .map(|endpoint| (endpoint.node_type, endpoint.id.as_str().to_string()))
        .collect()
}

#[test]
fn an_edge_row_relation_walks_out_and_back() {
    let records = fixture();
    assert_eq!(
        related(
            &records,
            RelationKind::References,
            NodeType::Source,
            "source_award",
            RelationDirection::Out,
        ),
        [(NodeType::Requirement, "req_overtime".to_string())]
    );
    assert_eq!(
        related(
            &records,
            RelationKind::References,
            NodeType::Requirement,
            "req_overtime",
            RelationDirection::In,
        ),
        [(NodeType::Source, "source_award".to_string())]
    );
}

#[test]
fn a_foreign_key_relation_walks_out_and_scans_back() {
    let records = fixture();
    assert_eq!(
        related(
            &records,
            RelationKind::RequirementInDomain,
            NodeType::Requirement,
            "req_overtime",
            RelationDirection::Out,
        ),
        [(NodeType::Domain, "domain_payroll".to_string())]
    );
    assert_eq!(
        related(
            &records,
            RelationKind::RequirementInDomain,
            NodeType::Domain,
            "domain_payroll",
            RelationDirection::In,
        ),
        [(NodeType::Requirement, "req_overtime".to_string())]
    );
    assert_eq!(
        related(
            &records,
            RelationKind::BoundaryConstrains,
            NodeType::Requirement,
            "req_overtime",
            RelationDirection::In,
        ),
        [(NodeType::Boundary, "boundary_no_backpay".to_string())]
    );
}

#[test]
fn an_embedded_collection_relation_walks_both_directions() {
    let records = fixture();
    assert_eq!(
        related(
            &records,
            RelationKind::RequirementCitesSource,
            NodeType::Requirement,
            "req_overtime",
            RelationDirection::Out,
        ),
        [(NodeType::Source, "source_award".to_string())]
    );
    assert_eq!(
        related(
            &records,
            RelationKind::RequirementCitesSource,
            NodeType::Source,
            "source_award",
            RelationDirection::In,
        ),
        [(NodeType::Requirement, "req_overtime".to_string())]
    );
    assert_eq!(
        related(
            &records,
            RelationKind::TopicLinks,
            NodeType::Rule,
            "rule_pay",
            RelationDirection::In,
        ),
        [(NodeType::Topic, "topic_rates".to_string())]
    );
}

#[test]
fn the_core_walks_every_declared_relation_around_one_node() {
    let records = fixture();
    let reached = related_nodes(
        &front(&records),
        NodeType::Requirement,
        &sid("req_overtime"),
    );
    let labels: Vec<(&str, RelationDirection, String)> = reached
        .iter()
        .map(|related| {
            (
                related.relation.name(),
                related.direction,
                related.endpoint.id.as_str().to_string(),
            )
        })
        .collect();
    assert_eq!(
        labels,
        [
            ("references", RelationDirection::In, "source_award".into()),
            (
                "boundary_constrains",
                RelationDirection::In,
                "boundary_no_backpay".to_string()
            ),
            ("topic_shapes", RelationDirection::In, "topic_rates".into()),
            (
                "requirement_in_domain",
                RelationDirection::Out,
                "domain_payroll".to_string()
            ),
            (
                "requirement_cites_source",
                RelationDirection::Out,
                "source_award".to_string()
            ),
        ]
    );
}
}
