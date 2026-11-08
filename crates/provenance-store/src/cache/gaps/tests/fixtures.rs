use super::super::{compute_gaps, GapGraph, GapItem, GapKind};
use provenance_core::{
    Question, QuestionStatus, Requirement, RequirementStatus, Resolution, ResolutionMethod,
    ResolutionStatus, Rule, RuleSeverity, RuleStatus, ScopeId, Source, SourceType, StableId, Topic,
    TopicStatus,
};

pub fn sid(value: &str) -> StableId {
    StableId::new(value).unwrap()
}

fn scope_id() -> ScopeId {
    ScopeId::new("default").unwrap()
}

pub fn requirement(id: &str) -> Requirement {
    Requirement {
        archived_in_commit: None,
        created: None,
        updated: None,
        scope_id: scope_id(),
        id: sid(id),
        declared_by: None,
        declaration_address: None,

        statement: format!("{id} statement"),
        description: None,
        status: RequirementStatus::Active,
        source_refs: Vec::new(),
        refines: Vec::new(),
        depends_on: Vec::new(),
        supersedes: Vec::new(),
        spawned_by: None,
        origin_thread: None,
        origin_message: None,
    }
}

pub fn resolution(id: &str) -> Resolution {
    Resolution {
        archived_in_commit: None,
        created: None,
        updated: None,
        scope_id: scope_id(),
        id: sid(id),
        title: id.to_string(),
        position: "Adopt the decision".to_string(),
        rationale: "It resolves the pair".to_string(),
        status: ResolutionStatus::Draft,
        context: None,
        inputs: Vec::new(),
        made_by: None,
        approved_by: None,
        approved_at: None,
        requirement_ids: Vec::new(),
        supersedes: Vec::new(),
        origin_thread: None,
        origin_message: None,
    }
}

pub fn rule(id: &str) -> Rule {
    Rule {
        created: None,
        updated: None,
        archived_in_commit: None,
        scope_id: scope_id(),
        id: sid(id),
        declared_by: None,
        declaration_address: None,

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
        scope_id: scope_id(),
        id: sid(id),
        requirement_id: sid(requirement_id),
        title: id.to_string(),
        status,
        archived_in_commit: None,
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
        depends_on: Vec::new(),
        scope_id: scope_id(),
        id: sid(id),
        topic_id: sid(topic_id),
        requirement_id: sid(requirement_id),
        question: "What remains?".to_string(),
        resolution_method: ResolutionMethod::Ask,
        status,
        archived_in_commit: None,
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
        created: None,
        updated: None,
        scope_id: scope_id(),
        id: sid(id),
        declared_by: None,
        declaration_address: None,

        name: id.to_string(),
        source_type: SourceType::Policy,
        url: None,
        reference: None,
        commit_pin: None,
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
        boundaries: &[],
        contributions: &[],
        synthesis_packets: &[],
    })
}

pub fn count_kind(gaps: &[GapItem], kind: GapKind) -> usize {
    gaps.iter().filter(|gap| gap.kind == kind).count()
}
