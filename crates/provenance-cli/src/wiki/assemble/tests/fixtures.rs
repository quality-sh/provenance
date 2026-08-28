use super::super::build_corpus;
use crate::handlers::ScopeExport;
use crate::wiki::links::LinkResolver;
use crate::wiki::model::GapKind;
use provenance_core::{
    Edge, EdgeType, Message, MessageRole, NodeType, Question, QuestionStatus, Requirement,
    RequirementStatus, Resolution, ResolutionInput, ResolutionInputType, ResolutionMethod,
    ResolutionStatus, Rule, RuleSeverity, RuleStatus, SchemaVersion, ScopeId, Source,
    SourceReference, SourceType, StableId, Thread, ThreadParent, ThreadStatus, Topic, TopicStatus,
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
        schema_version: SchemaVersion(1),
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
        origin_thread: None,
        origin_message: None,
    }
}

pub(super) fn resolution(id: &str, title: &str, inputs: Vec<ResolutionInput>) -> Resolution {
    Resolution {
        schema_version: SchemaVersion(1),
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
        superseded_by: None,
        review_on: None,
        origin_thread: None,
        origin_message: None,
    }
}

pub(super) fn rule(id: &str, name: Option<&str>) -> Rule {
    Rule {
        schema_version: SchemaVersion(1),
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
        origin_thread: None,
        origin_message: None,
    }
}

pub(super) fn source(id: &str, name: &str) -> Source {
    Source {
        schema_version: SchemaVersion(1),
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
        superseded_by: None,
        origin_thread: None,
        origin_message: None,
    }
}

pub(super) fn topic(id: &str, requirement_id: &str, status: TopicStatus) -> Topic {
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

pub(super) fn question(
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
        question: "What remains unresolved?".to_string(),
        resolution_method: ResolutionMethod::Grill,
        status,
        claimed_by: None,
        claimed_at: None,
        answer: None,
        links: Vec::new(),
        resolution_id: None,
    }
}

pub(super) fn edge(edge_type: EdgeType, from: (NodeType, &str), to: (NodeType, &str)) -> Edge {
    Edge {
        schema_version: SchemaVersion(1),
        scope_id: scope_id(),
        id: Edge::stable_id(edge_type, from.0, &sid(from.1), to.0, &sid(to.1)).unwrap(),
        edge_type,
        from_type: from.0,
        from_id: sid(from.1),
        to_type: to.0,
        to_id: sid(to.1),
        label: None,
    }
}

pub(super) fn thread(id: &str, parent: (NodeType, &str), created_at: i64) -> Thread {
    Thread {
        schema_version: SchemaVersion(1),
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
        schema_version: SchemaVersion(1),
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
        edges: vec![],
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

fn fixture_requirements() -> Vec<Requirement> {
    vec![
        requirement(
            "req_root",
            "Platform shall manage invoicing",
            RequirementStatus::Active,
            vec![],
        ),
        requirement(
            "req_child",
            "SaveInvoice shall split claim items",
            RequirementStatus::Resolved,
            vec![SourceReference {
                source_id: sid("source_schads"),
                clause: Some("clause 10.3".to_string()),
            }],
        ),
        requirement(
            "req_stuck",
            "Rostering shall respect awards",
            RequirementStatus::Resolved,
            vec![SourceReference {
                source_id: sid("source_missing"),
                clause: None,
            }],
        ),
    ]
}

fn fixture_resolutions() -> Vec<Resolution> {
    vec![
        resolution(
            "res_split",
            "Per-portion split",
            vec![ResolutionInput {
                input_type: ResolutionInputType::Technical,
                reference: "src/UseCase.php:59-69".to_string(),
                summary: "Codebase scan".to_string(),
            }],
        ),
        resolution("res_orphan", "Detached decision", vec![]),
    ]
}

fn fixture_rules() -> Vec<Rule> {
    vec![
        rule("rule_001", Some("Invoices grouped by participant")),
        rule("rule_orphan", None),
    ]
}

fn fixture_edges() -> Vec<Edge> {
    vec![
        edge(
            EdgeType::RefinesInto,
            (NodeType::Requirement, "req_root"),
            (NodeType::Requirement, "req_child"),
        ),
        edge(
            EdgeType::Resolves,
            (NodeType::Resolution, "res_split"),
            (NodeType::Requirement, "req_child"),
        ),
        edge(
            EdgeType::Produces,
            (NodeType::Resolution, "res_split"),
            (NodeType::Rule, "rule_001"),
        ),
        edge(
            EdgeType::Produces,
            (NodeType::Requirement, "req_child"),
            (NodeType::Rule, "rule_001"),
        ),
        edge(
            EdgeType::References,
            (NodeType::Source, "source_schads"),
            (NodeType::Requirement, "req_child"),
        ),
        edge(
            EdgeType::Spawns,
            (NodeType::Resolution, "res_split"),
            (NodeType::Requirement, "req_stuck"),
        ),
    ]
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
    state.sources = fixture_sources();
    state.requirements = fixture_requirements();
    state.resolutions = fixture_resolutions();
    state.rules = fixture_rules();
    state.edges = fixture_edges();
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
    let ideation_targets: Vec<(String, provenance_core::IdeationTarget)> = Vec::new();
    compute_gaps(&GapGraph {
        scope: &scope,
        sources: &state.sources,
        domains: &state.domains,
        boundaries: &state.boundaries,
        requirements: &state.requirements,
        resolutions: &state.resolutions,
        rules: &state.rules,
        topics: &state.topics,
        questions: &state.questions,
        edges: &state.edges,
        threads: &state.threads,
        ideation_targets: &ideation_targets,
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
