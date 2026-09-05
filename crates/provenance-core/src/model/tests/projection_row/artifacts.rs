use super::{assert_kind_round_trips, assert_round_trip};
use crate::model::{
    ColumnValue, DeclarationAddress, ProjectionRow, Requirement, RequirementStatus, Resolution,
    ResolutionInput, ResolutionInputType, ResolutionStatus, Rule, RuleSeverity, RuleStatus,
    ScopeId, Source, SourceReference, SourceType, StableId,
};
use crate::SUPPORTED_SCHEMA_VERSION;

fn sid(value: &str) -> StableId {
    StableId::new(value).unwrap()
}

fn scope() -> ScopeId {
    ScopeId::new("default").unwrap()
}

fn address() -> DeclarationAddress {
    DeclarationAddress::new(["src", "pay.rs", "overtime"]).unwrap()
}

fn filled_source() -> Source {
    Source {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: scope(),
        id: sid("source_schads"),
        declared_by: Some("spec://pay".into()),
        declaration_address: Some(address()),
        retired: true,
        name: "SCHADS award".into(),
        source_type: SourceType::Policy,
        url: Some("https://example.test/award".into()),
        reference: Some("clause 28".into()),
        commit_pin: Some("0123456789abcdef0123456789abcdef01234567".into()),
        effective_date: Some(1_700_000_000),
        review_date: Some(1_800_000_000),
        supersedes: vec![sid("source_award_2019")],
        origin_thread: Some(sid("thread_one")),
        origin_message: Some(sid("message_one")),
    }
}

fn bare_source() -> Source {
    Source {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: scope(),
        id: sid("source_bare"),
        declared_by: None,
        declaration_address: None,
        retired: false,
        name: "Bare".into(),
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

fn filled_requirement() -> Requirement {
    Requirement {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: scope(),
        id: sid("req_overtime"),
        declared_by: Some("spec://pay".into()),
        declaration_address: Some(address()),
        retired: true,
        statement: "Overtime is paid".into(),
        description: Some("After the threshold".into()),
        fog: Some("Which threshold".into()),
        status: RequirementStatus::Active,
        domain_id: Some(sid("domain_payroll")),
        source_refs: vec![
            SourceReference {
                source_id: sid("source_schads"),
                clause: Some("cl_1".into()),
            },
            SourceReference {
                source_id: sid("source_schads"),
                clause: None,
            },
        ],
        refines: Some(sid("req_pay")),
        depends_on: vec![sid("req_roster")],
        supersedes: vec![sid("req_old")],
        spawned_by: Some(sid("res_pay")),
        origin_thread: Some(sid("thread_one")),
        origin_message: Some(sid("message_one")),
    }
}

fn bare_requirement() -> Requirement {
    Requirement {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: scope(),
        id: sid("req_bare"),
        declared_by: None,
        declaration_address: None,
        retired: false,
        statement: "Bare".into(),
        description: None,
        fog: None,
        status: RequirementStatus::Discovery,
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

fn resolution(confidence: Option<f64>) -> Resolution {
    Resolution {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: scope(),
        id: sid("res_overtime"),
        title: "Overtime threshold".into(),
        position: "Use the award threshold".into(),
        rationale: "The award says so".into(),
        status: ResolutionStatus::Approved,
        context: Some("Payroll review".into()),
        enforcement: Some("A rule".into()),
        confidence,
        inputs: vec![ResolutionInput {
            input_type: ResolutionInputType::Regulatory,
            reference: "award.pdf".into(),
            summary: "The award text".into(),
        }],
        made_by: Some("ben".into()),
        approved_by: Some("ben".into()),
        approved_at: Some(1_700_000_000),
        requirement_ids: vec![sid("req_overtime")],
        supersedes: vec![sid("res_old")],
        review_on: Some("2027-01-01".into()),
        origin_thread: Some(sid("thread_one")),
        origin_message: Some(sid("message_one")),
    }
}

fn bare_resolution() -> Resolution {
    Resolution {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: scope(),
        id: sid("res_bare"),
        title: "Bare".into(),
        position: "Adopt".into(),
        rationale: "Because".into(),
        status: ResolutionStatus::Proposed,
        context: None,
        enforcement: None,
        confidence: None,
        inputs: Vec::new(),
        made_by: None,
        approved_by: None,
        approved_at: None,
        requirement_ids: Vec::new(),
        supersedes: Vec::new(),
        review_on: None,
        origin_thread: None,
        origin_message: None,
    }
}

fn filled_rule() -> Rule {
    Rule {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: scope(),
        id: sid("rule_overtime_001"),
        declared_by: Some("spec://pay".into()),
        declaration_address: Some(address()),
        retired: true,
        name: Some("Overtime threshold".into()),
        description: Some("Pay after the threshold".into()),
        statement: "Overtime is paid after the threshold".into(),
        status: RuleStatus::Active,
        severity: RuleSeverity::High,
        requirement_ids: vec![sid("req_overtime")],
        resolution_ids: vec![sid("res_overtime")],
        source_document: Some("award.pdf".into()),
        source_section: Some("28".into()),
        origin_thread: Some(sid("thread_one")),
        origin_message: Some(sid("message_one")),
    }
}

fn bare_rule() -> Rule {
    Rule {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: scope(),
        id: sid("rule_bare"),
        declared_by: None,
        declaration_address: None,
        retired: false,
        name: None,
        description: None,
        statement: "Bare".into(),
        status: RuleStatus::Draft,
        severity: RuleSeverity::Low,
        requirement_ids: Vec::new(),
        resolution_ids: Vec::new(),
        source_document: None,
        source_section: None,
        origin_thread: None,
        origin_message: None,
    }
}

#[test]
fn a_source_round_trips_through_its_row() {
    assert_kind_round_trips(&filled_source(), &bare_source());
}

#[test]
fn a_requirement_round_trips_through_its_row() {
    assert_kind_round_trips(&filled_requirement(), &bare_requirement());
}

#[test]
fn a_resolution_round_trips_through_its_row() {
    assert_kind_round_trips(&resolution(Some(0.95)), &bare_resolution());
}

#[test]
fn a_rule_round_trips_through_its_row() {
    assert_kind_round_trips(&filled_rule(), &bare_rule());
}

/// A confidence of exactly one is a real, not an integer: the column holds
/// `Real(1.0)` and the record prints `1.0` again.
#[test]
fn a_round_confidence_stays_a_float() {
    let record = resolution(Some(1.0));
    let row = record.row().unwrap();
    let index = Resolution::COLUMNS
        .iter()
        .position(|column| *column == "confidence")
        .unwrap();
    assert_eq!(row[index], ColumnValue::Real(1.0));
    assert_round_trip(&record);
    assert!(serde_json::to_string(&Resolution::from_row(&row).unwrap())
        .unwrap()
        .contains(r#""confidence":1.0"#));
}
