use super::super::build_corpus;
use crate::handlers::ScopeExport;
use crate::wiki::links::LinkResolver;
use crate::wiki::model::GapKind;
use provenance_core::SUPPORTED_SCHEMA_VERSION;
use provenance_core::{
    Message, MessageRole, NodeType, Question, QuestionStatus, Requirement, RequirementStatus,
    Resolution, ResolutionInput, ResolutionInputType, ResolutionMethod, ResolutionStatus, Rule,
    RuleSeverity, RuleStatus, ScopeId, Source, SourceReference, SourceType, StableId, Thread,
    ThreadParent, ThreadStatus, Topic, TopicStatus,
};
use provenance_store::cache::{compute_gaps, GapGraph, GapItem};

pub(super) fn sid(value: &str) -> StableId {
    StableId::new(value).unwrap()
}

pub(super) fn scope_id() -> ScopeId {
    ScopeId::new("default").unwrap()
}

pub(super) fn requirement(
    id: &str,
    statement: &str,
    status: RequirementStatus,
    source_refs: Vec<SourceReference>,
) -> Requirement {
    Requirement {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: scope_id(),
        id: sid(id),
        declared_by: None,
        declaration_address: None,
        retired: false,
        statement: statement.to_string(),
        description: None,
        fog: None,
        status,
        domain_id: Some(sid("domain_default")),
        source_refs,
        refines: None,
        depends_on: Vec::new(),
        supersedes: Vec::new(),
        spawned_by: None,
        origin_thread: None,
        origin_message: None,
    }
}

pub(super) fn resolution(id: &str, title: &str, inputs: Vec<ResolutionInput>) -> Resolution {
    Resolution {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: scope_id(),
        id: sid(id),
        title: title.to_string(),
        position: "Adopt the split".to_string(),
        rationale: "Atomicity equals drift detectability".to_string(),
        status: ResolutionStatus::Approved,
        context: Some("Codebase scan".to_string()),
        enforcement: Some("Specification".to_string()),
        confidence: Some(0.97),
        inputs,
        made_by: Some("Ben Nasraoui".to_string()),
        approved_by: Some("Ben Nasraoui".to_string()),
        approved_at: Some(1_745_000_000),
        review_on: None,
        requirement_ids: Vec::new(),
        supersedes: Vec::new(),
        origin_thread: None,
        origin_message: None,
    }
}

pub(super) fn rule(id: &str, name: Option<&str>) -> Rule {
    Rule {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: scope_id(),
        id: sid(id),
        declared_by: None,
        declaration_address: None,
        retired: false,
        name: name.map(str::to_string),
        description: None,
        statement: "Claim items shall be grouped by participant".to_string(),
        status: RuleStatus::Active,
        severity: RuleSeverity::High,
        source_document: Some("src/UseCase.php".to_string()),
        source_section: Some("59-69".to_string()),
        requirement_ids: Vec::new(),
        resolution_ids: Vec::new(),
        origin_thread: None,
        origin_message: None,
    }
}

pub(super) fn source(id: &str, name: &str) -> Source {
    Source {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: scope_id(),
        id: sid(id),
        declared_by: None,
        declaration_address: None,
        retired: false,
        name: name.to_string(),
        source_type: SourceType::Document,
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

pub(super) fn domain(id: &str, name: &str) -> provenance_core::Domain {
    provenance_core::Domain {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: scope_id(),
        id: sid(id),
        name: name.to_string(),
        description: Some(format!("About {name}")),
        color: None,
    }
}

pub(super) fn topic(id: &str, requirement_id: &str, status: TopicStatus) -> Topic {
    Topic {
        schema_version: SUPPORTED_SCHEMA_VERSION,
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

pub(super) fn question(
    id: &str,
    topic_id: &str,
    requirement_id: &str,
    status: QuestionStatus,
) -> Question {
    Question {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: scope_id(),
        id: sid(id),
        topic_id: sid(topic_id),
        requirement_id: sid(requirement_id),
        question: "What remains unresolved?".to_string(),
        resolution_method: ResolutionMethod::Grill,
        status,
        claimed_by: None,
        claimed_at: None,
        answer: None,
        links: Vec::new(),
        contradicts: None,
        resolution_id: None,
    }
}

pub(super) fn thread(id: &str, parent: (NodeType, &str), created_at: i64) -> Thread {
    Thread {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: scope_id(),
        id: sid(id),
        parent: ThreadParent {
            node_type: parent.0,
            node_id: sid(parent.1),
        },
        status: ThreadStatus::Active,
        created_at,
    }
}

pub(super) fn message(id: &str, thread_id: &str, body: &str, created_at: i64) -> Message {
    Message {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: scope_id(),
        id: sid(id),
        thread_id: sid(thread_id),
        role: MessageRole::Assistant,
        body: body.to_string(),
        created_at,
        ai_metadata: None,
    }
}

pub(super) fn empty_state() -> ScopeExport {
    ScopeExport {
        scope: "default".to_string(),
        sources: vec![],
        domains: vec![],
        requirements: vec![],
        boundaries: vec![],
        topics: vec![],
        questions: vec![],
        resolutions: vec![],
        rules: vec![],
        verification_bindings: vec![],
        implementation_bindings: vec![],
        threads: vec![],
        messages: vec![],
        contributions: vec![],
        synthesis_packets: vec![],
        proposal_cards: vec![],
        assertion_records: vec![],
        dispositions: vec![],
    }
}

fn fixture_sources() -> Vec<Source> {
    vec![
        {
            let mut schads = source("source_schads", "SCHADS Award mapping");
            schads.reference = Some("docs/award.md".to_string());
            schads.commit_pin = Some("abc1234".to_string());
            schads
        },
        source("source_unused", "Unused API spec"),
    ]
}

/// `req_child` refines `req_root`, cites `source_schads`, and is resolved
/// by `res_split`, which spawned `req_stuck`; `rule_001` names both the
/// requirement and the resolution.
fn fixture_requirements() -> Vec<Requirement> {
    let mut child = requirement(
        "req_child",
        "SaveInvoice shall split claim items",
        RequirementStatus::Resolved,
        vec![SourceReference {
            source_id: sid("source_schads"),
            clause: Some("clause 10.3".to_string()),
        }],
    );
    child.refines = Some(sid("req_root"));
    let mut stuck = requirement(
        "req_stuck",
        "Rostering shall respect awards",
        RequirementStatus::Resolved,
        vec![SourceReference {
            source_id: sid("source_missing"),
            clause: None,
        }],
    );
    stuck.spawned_by = Some(sid("res_split"));
    vec![
        requirement(
            "req_root",
            "Platform shall manage invoicing",
            RequirementStatus::Active,
            vec![],
        ),
        child,
        stuck,
    ]
}

fn fixture_resolutions() -> Vec<Resolution> {
    let mut split = resolution(
        "res_split",
        "Per-portion split",
        vec![ResolutionInput {
            input_type: ResolutionInputType::Technical,
            reference: "src/UseCase.php:59-69".to_string(),
            summary: "Codebase scan".to_string(),
        }],
    );
    split.requirement_ids = vec![sid("req_child")];
    vec![split, resolution("res_orphan", "Detached decision", vec![])]
}

fn fixture_rules() -> Vec<Rule> {
    let mut grouped = rule("rule_001", Some("Invoices grouped by participant"));
    grouped.requirement_ids = vec![sid("req_child")];
    grouped.resolution_ids = vec![sid("res_split")];
    vec![grouped, rule("rule_orphan", None)]
}

fn fixture_threads() -> Vec<Thread> {
    vec![
        thread("thr_req_child", (NodeType::Requirement, "req_child"), 10),
        thread("thr_res_split", (NodeType::Resolution, "res_split"), 20),
    ]
}

fn fixture_messages() -> Vec<Message> {
    vec![
        message("msg_scoping", "thr_req_child", "Scoping note", 1),
        message(
            "msg_guard",
            "thr_res_split",
            "Guard at src/UseCase.php:153-156 confirmed by testCreateGapInvoiceOnly.",
            2,
        ),
    ]
}

pub(super) fn fixture_state() -> ScopeExport {
    let mut state = empty_state();
    state.domains = vec![domain("domain_default", "Invoicing")];
    state.sources = fixture_sources();
    state.requirements = fixture_requirements();
    state.resolutions = fixture_resolutions();
    state.rules = fixture_rules();
    state.threads = fixture_threads();
    state.messages = fixture_messages();
    state
}

pub(super) fn fixture_corpus() -> crate::wiki::model::WikiCorpus {
    let resolver = LinkResolver::new(Some("git@github.com:exampleorg/ex-api.git"));
    build_corpus(&fixture_state(), &resolver)
}

pub(super) fn gap_kinds(gaps: &[crate::wiki::model::GapNotice]) -> Vec<GapKind> {
    gaps.iter().map(|gap| gap.kind).collect()
}

pub(super) fn compute_state_gaps(state: &ScopeExport) -> Vec<GapItem> {
    let scope = scope_id();
    compute_gaps(&GapGraph {
        scope: &scope,
        sources: &state.sources,
        requirements: &state.requirements,
        resolutions: &state.resolutions,
        rules: &state.rules,
        topics: &state.topics,
        questions: &state.questions,
        threads: &state.threads,
        domains: &state.domains,
        boundaries: &state.boundaries,
    })
}

pub(super) fn requirement_page<'a>(
    corpus: &'a crate::wiki::model::WikiCorpus,
    id: &str,
) -> &'a crate::wiki::model::RequirementPage {
    corpus
        .requirements
        .iter()
        .find(|page| page.id.record_id == id)
        .unwrap()
}

pub(super) fn resolution_page<'a>(
    corpus: &'a crate::wiki::model::WikiCorpus,
    id: &str,
) -> &'a crate::wiki::model::ResolutionPage {
    corpus
        .resolutions
        .iter()
        .find(|page| page.id.record_id == id)
        .unwrap()
}

pub(super) fn rule_page<'a>(
    corpus: &'a crate::wiki::model::WikiCorpus,
    id: &str,
) -> &'a crate::wiki::model::RulePage {
    corpus
        .rules
        .iter()
        .find(|page| page.id.record_id == id)
        .unwrap()
}

pub(super) fn source_page<'a>(
    corpus: &'a crate::wiki::model::WikiCorpus,
    id: &str,
) -> &'a crate::wiki::model::SourcePage {
    corpus
        .sources
        .iter()
        .find(|page| page.id.record_id == id)
        .unwrap()
}
