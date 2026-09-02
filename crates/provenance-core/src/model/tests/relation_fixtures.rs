mod relation_fixtures {
use crate::model::relations::{RecordFront, RelationDirection, RelationKind, RelationSource};
use crate::model::{
    ArtifactLink, ArtifactLinkTargetType, Boundary, Domain, Edge, EdgeType, NodeType, Question,
    QuestionStatus, Requirement, RequirementStatus, Resolution, ResolutionMethod,
    ResolutionStatus, Rule, RuleSeverity, RuleStatus, ScopeId, SchemaVersion, Source,
    SourceReference, SourceType, StableId, Topic, TopicStatus,
};

pub(super) fn scope() -> ScopeId {
    ScopeId::new("default").unwrap()
}

pub(super) fn sid(value: &str) -> StableId {
    StableId::new(value).unwrap()
}

pub(super) fn source(id: &str) -> Source {
    source_superseded_by(id, None)
}

pub(super) fn source_superseded_by(id: &str, successor: Option<&str>) -> Source {
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
        superseded_by: successor.map(sid),
        origin_thread: None,
        origin_message: None,
    }
}

pub(super) fn requirement(id: &str, domain_id: Option<&str>, cites: &[&str]) -> Requirement {
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

pub(super) fn domain(id: &str) -> Domain {
    Domain {
        schema_version: SchemaVersion(1),
        scope_id: scope(),
        id: sid(id),
        name: id.to_string(),
        description: None,
        color: None,
    }
}

pub(super) fn boundary(id: &str, requirement_id: &str) -> Boundary {
    boundary_citing(id, requirement_id, None)
}

pub(super) fn boundary_citing(id: &str, requirement_id: &str, source: Option<&str>) -> Boundary {
    Boundary {
        schema_version: SchemaVersion(1),
        scope_id: scope(),
        id: sid(id),
        requirement_id: sid(requirement_id),
        statement: format!("{id} statement"),
        source_ref: source.map(|source_id| SourceReference {
            source_id: sid(source_id),
            clause: None,
        }),
    }
}

pub(super) fn topic(id: &str, requirement_id: &str, links: &[(&str, ArtifactLinkTargetType)]) -> Topic {
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

pub(super) fn rule(id: &str) -> Rule {
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

pub(super) fn edge(id: &str, edge_type: EdgeType, from: (NodeType, &str), to: (NodeType, &str)) -> Edge {
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

pub(super) struct Fixture {
    pub(super) sources: Vec<Source>,
    pub(super) requirements: Vec<Requirement>,
    pub(super) domains: Vec<Domain>,
    pub(super) boundaries: Vec<Boundary>,
    pub(super) topics: Vec<Topic>,
    pub(super) questions: Vec<Question>,
    pub(super) resolutions: Vec<Resolution>,
    pub(super) rules: Vec<Rule>,
    pub(super) edges: Vec<Edge>,
}

pub(super) fn resolution(id: &str) -> Resolution {
    resolution_superseded_by(id, None)
}

pub(super) fn resolution_superseded_by(id: &str, successor: Option<&str>) -> Resolution {
    Resolution {
        schema_version: SchemaVersion(1),
        scope_id: scope(),
        id: sid(id),
        title: id.to_string(),
        position: "Position".into(),
        rationale: "Rationale".into(),
        status: ResolutionStatus::Proposed,
        context: None,
        enforcement: None,
        confidence: None,
        inputs: Vec::new(),
        made_by: None,
        approved_by: None,
        approved_at: None,
        superseded_by: successor.map(sid),
        review_on: None,
        origin_thread: None,
        origin_message: None,
    }
}

pub(super) fn question(
    id: &str,
    topic_id: &str,
    requirement_id: &str,
    resolution_id: Option<&str>,
    links: &[(&str, ArtifactLinkTargetType)],
) -> Question {
    Question {
        schema_version: SchemaVersion(1),
        scope_id: scope(),
        id: sid(id),
        topic_id: sid(topic_id),
        requirement_id: sid(requirement_id),
        question: format!("{id}?"),
        resolution_method: ResolutionMethod::Grill,
        status: QuestionStatus::Open,
        claimed_by: None,
        claimed_at: None,
        answer: None,
        links: links
            .iter()
            .map(|(target_id, target_type)| ArtifactLink {
                target_type: *target_type,
                target_id: sid(target_id),
            })
            .collect(),
        resolution_id: resolution_id.map(sid),
    }
}

pub(super) fn fixture() -> Fixture {
    let req = |edge_type: EdgeType, to: (NodeType, &str)| {
        edge(
            &format!("edge_{}", RelationKindName(edge_type).name()),
            edge_type,
            (NodeType::Requirement, "req_overtime"),
            to,
        )
    };
    Fixture {
        sources: vec![
            source("source_award"),
            source_superseded_by("source_award_2019", Some("source_award")),
        ],
        requirements: vec![
            requirement("req_overtime", Some("domain_payroll"), &["source_award"]),
            requirement("req_penalty", None, &[]),
        ],
        domains: vec![domain("domain_payroll")],
        boundaries: vec![boundary_citing(
            "boundary_no_backpay",
            "req_overtime",
            Some("source_award"),
        )],
        topics: vec![topic(
            "topic_rates",
            "req_overtime",
            &[("rule_pay", ArtifactLinkTargetType::Rule)],
        )],
        questions: vec![question(
            "question_threshold",
            "topic_rates",
            "req_overtime",
            Some("res_threshold"),
            &[("source_award", ArtifactLinkTargetType::Source)],
        )],
        resolutions: vec![
            resolution("res_threshold"),
            resolution_superseded_by("res_threshold_draft", Some("res_threshold")),
        ],
        rules: vec![rule("rule_pay")],
        edges: vec![
            edge(
                "edge_cite",
                EdgeType::References,
                (NodeType::Source, "source_award"),
                (NodeType::Requirement, "req_overtime"),
            ),
            req(EdgeType::RefinesInto, (NodeType::Requirement, "req_penalty")),
            req(EdgeType::DependsOn, (NodeType::Requirement, "req_penalty")),
            req(EdgeType::Contradicts, (NodeType::Requirement, "req_penalty")),
            req(EdgeType::Supersedes, (NodeType::Requirement, "req_penalty")),
            req(EdgeType::Needs, (NodeType::Resolution, "res_threshold")),
            req(EdgeType::Produces, (NodeType::Rule, "rule_pay")),
            edge(
                "edge_resolves",
                EdgeType::Resolves,
                (NodeType::Resolution, "res_threshold"),
                (NodeType::Requirement, "req_overtime"),
            ),
            edge(
                "edge_spawns",
                EdgeType::Spawns,
                (NodeType::Resolution, "res_threshold"),
                (NodeType::Requirement, "req_overtime"),
            ),
        ],
    }
}

/// The edge type's wire name, for unique edge ids in the corpus.
struct RelationKindName(EdgeType);

impl RelationKindName {
    fn name(&self) -> &'static str {
        match self.0 {
            EdgeType::References => "references",
            EdgeType::RefinesInto => "refines_into",
            EdgeType::DependsOn => "depends_on",
            EdgeType::Contradicts => "contradicts",
            EdgeType::Supersedes => "supersedes",
            EdgeType::Needs => "needs",
            EdgeType::Resolves => "resolves",
            EdgeType::Spawns => "spawns",
            EdgeType::Produces => "produces",
        }
    }
}

pub(super) fn front(records: &Fixture) -> RecordFront<'_> {
    RecordFront {
        sources: &records.sources,
        requirements: &records.requirements,
        resolutions: &records.resolutions,
        rules: &records.rules,
        topics: &records.topics,
        questions: &records.questions,
        domains: &records.domains,
        boundaries: &records.boundaries,
        edges: &records.edges,
    }
}

pub(super) fn related(
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
}
