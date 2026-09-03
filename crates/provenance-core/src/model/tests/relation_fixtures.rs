mod relation_fixtures {
use crate::model::relations::RecordFront;
use crate::model::{
    ArtifactLink, ArtifactLinkTargetType, Boundary, Domain, Question, QuestionStatus,
    Requirement, RequirementStatus, Resolution, ResolutionMethod, ResolutionStatus, Rule,
    RuleSeverity, RuleStatus, ScopeId, Source, SourceReference, SourceType,
    StableId, Topic, TopicStatus,
};
use crate::SUPPORTED_SCHEMA_VERSION;

pub(super) fn scope() -> ScopeId {
    ScopeId::new("default").unwrap()
}

pub(super) fn sid(value: &str) -> StableId {
    StableId::new(value).unwrap()
}

pub(super) fn ids(values: &[&str]) -> Vec<StableId> {
    values.iter().map(|value| sid(value)).collect()
}

pub(super) fn source(id: &str, supersedes: &[&str]) -> Source {
    Source {
        schema_version: SUPPORTED_SCHEMA_VERSION,
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
        supersedes: ids(supersedes),
        origin_thread: None,
        origin_message: None,
    }
}

pub(super) fn requirement(id: &str, domain_id: Option<&str>, cites: &[&str]) -> Requirement {
    Requirement {
        schema_version: SUPPORTED_SCHEMA_VERSION,
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
        refines: None,
        depends_on: Vec::new(),
        supersedes: Vec::new(),
        spawned_by: None,
        origin_thread: None,
        origin_message: None,
    }
}

pub(super) fn domain(id: &str) -> Domain {
    Domain {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: scope(),
        id: sid(id),
        name: id.to_string(),
        description: None,
        color: None,
    }
}

pub(super) fn boundary_citing(id: &str, requirement_id: &str, source: Option<&str>) -> Boundary {
    Boundary {
        schema_version: SUPPORTED_SCHEMA_VERSION,
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
        schema_version: SUPPORTED_SCHEMA_VERSION,
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

pub(super) fn rule(id: &str, requirements: &[&str], resolutions: &[&str]) -> Rule {
    Rule {
        schema_version: SUPPORTED_SCHEMA_VERSION,
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
        requirement_ids: ids(requirements),
        resolution_ids: ids(resolutions),
        source_document: None,
        source_section: None,
        origin_thread: None,
        origin_message: None,
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
}

pub(super) fn resolution(id: &str, requirements: &[&str], supersedes: &[&str]) -> Resolution {
    Resolution {
        schema_version: SUPPORTED_SCHEMA_VERSION,
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
        requirement_ids: ids(requirements),
        supersedes: ids(supersedes),
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
    contradicts: Option<&str>,
    links: &[(&str, ArtifactLinkTargetType)],
) -> Question {
    Question {
        schema_version: SUPPORTED_SCHEMA_VERSION,
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
        contradicts: contradicts.map(sid),
    }
}

/// One record of every kind, touching every declared relation at least
/// once: `req_overtime` is the hub, `req_penalty` refines and depends on it.
pub(super) fn fixture() -> Fixture {
    let mut penalty = requirement("req_penalty", None, &[]);
    penalty.refines = Some(sid("req_overtime"));
    penalty.depends_on = ids(&["req_overtime"]);
    let mut overtime = requirement("req_overtime", Some("domain_payroll"), &["source_award"]);
    overtime.spawned_by = Some(sid("res_threshold"));
    overtime.supersedes = ids(&["req_penalty"]);
    Fixture {
        sources: vec![
            source("source_award", &["source_award_2019"]),
            source("source_award_2019", &[]),
        ],
        requirements: vec![overtime, penalty],
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
            Some("req_penalty"),
            &[("source_award", ArtifactLinkTargetType::Source)],
        )],
        resolutions: vec![
            resolution("res_threshold", &["req_overtime"], &["res_threshold_draft"]),
            resolution("res_threshold_draft", &["req_overtime"], &[]),
        ],
        rules: vec![rule("rule_pay", &["req_overtime"], &["res_threshold"])],
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
    }
}
}
