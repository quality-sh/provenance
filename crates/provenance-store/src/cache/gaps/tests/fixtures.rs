use super::super::{compute_gaps, GapGraph, GapItem, GapKind};
use provenance_core::{
    NodeType, Question, QuestionStatus, Requirement, RequirementStatus, Resolution,
    ResolutionMethod, ResolutionStatus, Rule, RuleSeverity, RuleStatus, SchemaVersion, ScopeId,
    Source, SourceType, StableId, Topic, TopicStatus,
};

pub fn sid(value: &str) -> StableId {
    StableId::new(value).unwrap()
}

fn scope_id() -> ScopeId {
    ScopeId::new("default").unwrap()
}

pub fn requirement(id: &str) -> Requirement {
    Requirement {
        schema_version: SchemaVersion(1),
        scope_id: scope_id(),
        id: sid(id),
        declared_by: None,
        declaration_address: None,
        retired: false,
        statement: format!("{id} statement"),
        description: None,
        fog: None,
        status: RequirementStatus::Active,
        domain_id: None,
        source_refs: Vec::new(),
        refines: None,
        depends_on: Vec::new(),
        supersedes: Vec::new(),
        spawned_by: None,
        origin_thread: None,
        origin_message: None,
    }
}

pub fn resolution(id: &str) -> Resolution {
    Resolution {
        schema_version: SchemaVersion(1),
        scope_id: scope_id(),
        id: sid(id),
        title: id.to_string(),
        position: "Adopt the decision".to_string(),
        rationale: "It resolves the pair".to_string(),
        status: ResolutionStatus::Draft,
        context: None,
        enforcement: None,
        confidence: None,
        inputs: Vec::new(),
        made_by: None,
        approved_by: None,
        approved_at: None,
        review_on: None,
        requirement_ids: Vec::new(),
        supersedes: Vec::new(),
        origin_thread: None,
        origin_message: None,
    }
}

pub fn rule(id: &str) -> Rule {
    Rule {
        schema_version: SchemaVersion(1),
        scope_id: scope_id(),
        id: sid(id),
        declared_by: None,
        declaration_address: None,
        retired: false,
        name: None,
        description: None,
        statement: "Rule statement".to_string(),
        status: RuleStatus::Active,
        severity: RuleSeverity::High,
        source_document: None,
        source_section: None,
        requirement_ids: Vec::new(),
        resolution_ids: Vec::new(),
        origin_thread: None,
        origin_message: None,
    }
}

pub fn topic(id: &str, status: TopicStatus) -> Topic {
    topic_for(id, "req_topic", status)
}

pub fn topic_for(id: &str, requirement_id: &str, status: TopicStatus) -> Topic {
    Topic {
        schema_version: SchemaVersion(1),
        scope_id: scope_id(),
        id: sid(id),
        requirement_id: sid(requirement_id),
        title: id.to_string(),
        status,
        claimed_by: None,
        claimed_at: None,
        links: Vec::new(),
    }
}

pub fn question(id: &str, topic_id: &str, status: QuestionStatus) -> Question {
    question_for(id, topic_id, "req_topic", status)
}

pub fn question_for(
    id: &str,
    topic_id: &str,
    requirement_id: &str,
    status: QuestionStatus,
) -> Question {
    Question {
        schema_version: SchemaVersion(1),
        scope_id: scope_id(),
        id: sid(id),
        topic_id: sid(topic_id),
        requirement_id: sid(requirement_id),
        question: "What remains?".to_string(),
        resolution_method: ResolutionMethod::Grill,
        status,
        claimed_by: None,
        claimed_at: None,
        answer: (status == QuestionStatus::Answered).then(|| "Done".to_string()),
        links: Vec::new(),
        contradicts: None,
        resolution_id: None,
    }
}

pub fn source(id: &str) -> Source {
    Source {
        schema_version: SchemaVersion(1),
        scope_id: scope_id(),
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
        supersedes: Vec::new(),
        origin_thread: None,
        origin_message: None,
    }
}

pub fn compute_for(
    sources: &[Source],
    requirements: &[Requirement],
    resolutions: &[Resolution],
    rules: &[Rule],
    topics: &[Topic],
    questions: &[Question],
    domains: &[provenance_core::Domain],
) -> Vec<GapItem> {
    let scope = scope_id();
    compute_gaps(&GapGraph {
        scope: &scope,
        sources,
        requirements,
        resolutions,
        rules,
        topics,
        questions,
        threads: &[],
        domains,
        boundaries: &[],
    })
}

pub fn domain(id: &str) -> provenance_core::Domain {
    provenance_core::Domain {
        schema_version: SchemaVersion(1),
        scope_id: scope_id(),
        id: sid(id),
        name: id.to_string(),
        description: None,
        color: None,
    }
}

pub fn thread(id: &str, parent_type: NodeType, parent_id: &str) -> provenance_core::Thread {
    provenance_core::Thread {
        schema_version: SchemaVersion(1),
        scope_id: scope_id(),
        id: sid(id),
        parent: provenance_core::ThreadParent {
            node_type: parent_type,
            node_id: sid(parent_id),
        },
        status: provenance_core::ThreadStatus::Active,
        created_at: 1,
    }
}

pub fn count_kind(gaps: &[GapItem], kind: GapKind) -> usize {
    gaps.iter().filter(|gap| gap.kind == kind).count()
}
