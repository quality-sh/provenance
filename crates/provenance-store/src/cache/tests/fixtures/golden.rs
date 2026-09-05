//! The frozen corpus behind the golden test and the differential harness:
//! every kind and every relation, retired records that reference and are
//! referenced, a diamond over a retired requirement, a source cited under
//! two clauses, a `links` pair naming one id under two kinds, lists past
//! the limit, a cleared review, and one source file with scanner sites.
//! It never reads `.provenance/state`, which moves on most pull requests.

use super::{attach_source, create_resolution, create_rule_of, create_source, empty_layout, sid};
use crate::layout::ProvenanceLayout;
use crate::shards;
use crate::state_store::{
    AddSourceReferenceInput, CreateBoundaryInput, CreateDomainInput, CreateQuestionInput,
    CreateRequirementInput, CreateSourceInput, CreateTopicInput, StateStore,
};
use provenance_core::{
    ArtifactLink, ArtifactLinkTargetType, QuestionStatus, RequirementStatus, ResolutionMethod,
    ScopeId, SourceReference, SourceType, TopicStatus, SUPPORTED_SCHEMA_VERSION,
};

/// The id one requirement and one rule share, so `links` can name it
/// under two kinds.
pub const TWIN_ID: &str = "twin_record";

pub fn golden_layout() -> (tempfile::TempDir, ProvenanceLayout, ScopeId) {
    let (dir, layout, scope) = empty_layout();
    let store = StateStore::new(layout.clone());
    seed_graph(&store, &scope);
    seed_shaping(&store, &scope);
    seed_integrations(&layout, &scope);
    for id in ["req_old_overtime", "req_right"] {
        mark_retired(&shards::requirements_path(&layout, &scope), id);
    }
    mark_retired(
        &shards::sources_path(&layout, &scope),
        "source_retired_note",
    );
    let source = layout.root().join("src/pay.rs");
    std::fs::create_dir_all(source.parent().unwrap()).unwrap();
    std::fs::write(
        source,
        "#[rule(\"rule_overtime_001\")]\nfn pay() {}\n\n#[verifies(\"rule_overtime_001\", examples)]\nfn pay_examples() {}\n",
    )
    .unwrap();
    (dir, layout, scope)
}

fn requirement(id: &str, refines: Option<&str>, depends_on: &[&str]) -> CreateRequirementInput {
    CreateRequirementInput {
        scope_id: ScopeId::new("default").unwrap(),
        id: sid(id),
        statement: format!("{} overtime statement", id.replace('_', " ")),
        description: None,
        status: RequirementStatus::Active,
        domain_id: Some(sid("domain_payroll")),
        refines: refines.map(sid),
        depends_on: depends_on.iter().map(|id| sid(id)).collect(),
        supersedes: Vec::new(),
        spawned_by: None,
        origin_thread: None,
        origin_message: None,
    }
}

fn seed_graph(store: &StateStore, scope: &ScopeId) {
    store
        .create_domain(CreateDomainInput {
            scope_id: scope.clone(),
            id: sid("domain_payroll"),
            name: "Payroll".into(),
            description: Some("Wages and overtime".into()),
            color: None,
        })
        .unwrap();
    create_source(store, scope, "source_award_2019");
    create_source(store, scope, "source_retired_note");
    store
        .create_source(CreateSourceInput {
            scope_id: scope.clone(),
            id: sid("source_schads"),
            name: "SCHADS award".into(),
            source_type: SourceType::Policy,
            url: None,
            reference: Some("clause 28".into()),
            commit_pin: None,
            effective_date: None,
            review_date: None,
            supersedes: vec![sid("source_award_2019")],
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
    store
        .create_requirement(requirement("req_overtime", None, &[]))
        .unwrap();
    for clause in ["cl_1", "cl_2"] {
        store
            .add_source_reference(AddSourceReferenceInput {
                scope_id: scope.clone(),
                source_id: sid("source_schads"),
                requirement_id: sid("req_overtime"),
                clause: Some(clause.into()),
            })
            .unwrap();
    }
    attach_source(store, scope, "req_overtime", "source_retired_note");
    store
        .create_requirement(requirement("req_old_overtime", Some("req_overtime"), &[]))
        .unwrap();
    store
        .create_requirement(CreateRequirementInput {
            supersedes: vec![sid("req_old_overtime")],
            ..requirement("req_penalty", Some("req_overtime"), &["req_overtime"])
        })
        .unwrap();
    store
        .create_requirement(requirement("req_top", None, &[]))
        .unwrap();
    store
        .create_requirement(requirement("req_left", Some("req_top"), &[]))
        .unwrap();
    store
        .create_requirement(requirement("req_right", Some("req_top"), &[]))
        .unwrap();
    store
        .create_requirement(requirement("req_bottom", None, &["req_left", "req_right"]))
        .unwrap();
    store
        .create_requirement(requirement(TWIN_ID, Some("req_top"), &[]))
        .unwrap();
    create_resolution(store, scope, "res_overtime", "req_overtime");
    create_resolution(store, scope, "res_penalty", "req_penalty");
    store
        .create_rule(crate::state_store::CreateRuleInput {
            scope_id: scope.clone(),
            id: sid("rule_overtime_001"),
            name: Some("Overtime threshold".into()),
            description: Some("Pay overtime after the threshold".into()),
            requirement_ids: vec![sid("req_overtime")],
            resolution_ids: vec![sid("res_overtime")],
            statement: "Overtime is paid after the threshold".into(),
            status: provenance_core::RuleStatus::Active,
            severity: provenance_core::RuleSeverity::High,
            source_document: None,
            source_section: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
    for index in 2..=9 {
        create_rule_of(
            store,
            scope,
            &format!("rule_over_{index:03}"),
            "req_overtime",
        );
    }
    create_rule_of(store, scope, "rule_penalty_001", "req_penalty");
    create_rule_of(store, scope, "rule_old_001", "req_old_overtime");
    create_rule_of(store, scope, TWIN_ID, "req_top");
}

fn seed_shaping(store: &StateStore, scope: &ScopeId) {
    let link = |target_type, id: &str| ArtifactLink {
        target_type,
        target_id: sid(id),
    };
    store
        .create_topic(CreateTopicInput {
            scope_id: scope.clone(),
            id: sid("topic_rates"),
            requirement_id: sid("req_overtime"),
            title: "Rates".into(),
            status: TopicStatus::Open,
            links: vec![
                link(ArtifactLinkTargetType::Requirement, TWIN_ID),
                link(ArtifactLinkTargetType::Rule, TWIN_ID),
                link(ArtifactLinkTargetType::Source, "source_schads"),
            ],
        })
        .unwrap();
    store
        .create_question(CreateQuestionInput {
            scope_id: scope.clone(),
            id: sid("question_threshold"),
            topic_id: sid("topic_rates"),
            question: "Which threshold applies to overtime?".into(),
            resolution_method: ResolutionMethod::Grill,
            status: QuestionStatus::Open,
            answer: None,
            links: vec![link(ArtifactLinkTargetType::Rule, "rule_overtime_001")],
            resolution_id: Some(sid("res_overtime")),
            contradicts: Some(sid("req_penalty")),
        })
        .unwrap();
    store
        .create_boundary(CreateBoundaryInput {
            scope_id: scope.clone(),
            id: sid("boundary_no_backpay"),
            requirement_id: sid("req_overtime"),
            statement: "Back pay is out of scope".into(),
            source_ref: Some(SourceReference {
                source_id: sid("source_schads"),
                clause: Some("cl_3".into()),
            }),
        })
        .unwrap();
}

/// Bindings and reviews go in as raw shard lines, past the writers, in
/// the shape the integration records serialize to.
fn seed_integrations(layout: &ProvenanceLayout, scope: &ScopeId) {
    let version = SUPPORTED_SCHEMA_VERSION.0;
    let scope_word = scope.as_str();
    let implementations = [
        (
            "bind_impl_a",
            "rule_overtime_001",
            "src/pay.rs",
            "pay",
            false,
        ),
        (
            "bind_impl_b",
            "rule_overtime_001",
            "src/rates.rs",
            "rate",
            false,
        ),
        (
            "bind_impl_c",
            "rule_overtime_001",
            "src/audit.rs",
            "audit",
            true,
        ),
        (
            "bind_impl_d",
            "rule_penalty_001",
            "src/rates.rs",
            "penalty",
            false,
        ),
    ];
    let lines: Vec<String> = implementations
        .iter()
        .map(|(id, rule, file, symbol, retired)| {
            format!(
                r#"{{"schema_version":{version},"scope_id":"{scope_word}","id":"{id}","rule_id":"{rule}","declared_by":"spec://golden","retired":{retired},"file":"{file}","symbol":"{symbol}"}}"#
            )
        })
        .collect();
    write_lines(&shards::implementation_bindings_path(layout, scope), &lines);
    let verifications = [
        (
            "bind_ver_a",
            "rule_overtime_001",
            "pay_examples",
            "src/pay.rs",
            "pay_examples",
        ),
        (
            "bind_ver_b",
            "rule_overtime_001",
            "pay_property",
            "tests/pay.rs",
            "pay_holds",
        ),
        (
            "bind_ver_c",
            "rule_overtime_001",
            "pay_conformance",
            "tests/pay.rs",
            "pay_agrees",
        ),
        (
            "bind_ver_d",
            "rule_penalty_001",
            "penalty_examples",
            "tests/rates.rs",
            "penalty_works",
        ),
    ];
    let lines: Vec<String> = verifications
        .iter()
        .map(|(id, rule, key, file, symbol)| {
            format!(
                r#"{{"schema_version":{version},"scope_id":"{scope_word}","id":"{id}","rule_id":"{rule}","key":"{key}","method":"examples","declared_by":"spec://golden","file":"{file}","symbol":"{symbol}"}}"#
            )
        })
        .collect();
    write_lines(&shards::verification_bindings_path(layout, scope), &lines);
    let reviews = [
        ("review_a", "rule_overtime_001", 1, ""),
        (
            "review_b",
            "rule_overtime_001",
            2,
            r#","cleared_at":5,"cleared_by_run":"run_cleared""#,
        ),
        ("review_c", "rule_overtime_001", 3, ""),
        ("review_d", "rule_penalty_001", 4, ""),
    ];
    let lines: Vec<String> = reviews
        .iter()
        .map(|(id, rule, changed_at, cleared)| {
            format!(
                r#"{{"schema_version":{version},"scope_id":"{scope_word}","id":"{id}","rule_id":"{rule}","requirement_id":"req_overtime","field":"statement","before":"Overtime","after":"Overtime pay","changed_at":{changed_at}{cleared}}}"#
            )
        })
        .collect();
    write_lines(&shards::requirement_reviews_path(layout, scope), &lines);
}

fn write_lines(path: &camino::Utf8Path, lines: &[String]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, format!("{}\n", lines.join("\n"))).unwrap();
}

/// Marks one record retired in place, as a retire would leave it.
pub fn mark_retired(path: &camino::Utf8Path, id: &str) {
    let rewritten: Vec<String> = std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|line| {
            let mut record: serde_json::Value = serde_json::from_str(line).unwrap();
            if record["id"] == id {
                record["retired"] = serde_json::Value::Bool(true);
            }
            record.to_string()
        })
        .collect();
    write_lines(path, &rewritten);
}
