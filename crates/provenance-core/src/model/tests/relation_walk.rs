mod relation_walk {
use crate::model::relations::RelationOwner;
use crate::model::{
    ArtifactLink, ArtifactLinkTargetType, Boundary, DeclarationAddress, Question, QuestionStatus,
    Requirement, RequirementStatus, Resolution, ResolutionInput, ResolutionInputType,
    ResolutionMethod, ResolutionStatus, Rule, RuleSeverity, RuleStatus, ScopeId,
    Source, SourceReference, SourceType, StableId, Topic, TopicStatus,
};
use provenance_macros::verifies;
use serde_json::Value;
use std::collections::BTreeSet;
use crate::SUPPORTED_SCHEMA_VERSION;

fn sid(value: &str) -> StableId {
    StableId::new(value).unwrap()
}

fn reference(source: &str) -> SourceReference {
    SourceReference {
        source_id: sid(source),
        clause: Some("clause".into()),
    }
}

fn link() -> ArtifactLink {
    ArtifactLink {
        target_type: ArtifactLinkTargetType::Rule,
        target_id: sid("rule_pay"),
    }
}

/// Every key ending `_id` or `_ids`, plus every declared name, at any depth.
fn reference_keys(value: &Value, declared: &[&str], keys: &mut BTreeSet<String>) {
    match value {
        Value::Object(fields) => {
            for (key, nested) in fields {
                if key.ends_with("_id") || key.ends_with("_ids") || declared.contains(&key.as_str()) {
                    keys.insert(key.clone());
                }
                reference_keys(nested, declared, keys);
            }
        }
        Value::Array(items) => {
            for item in items {
                reference_keys(item, declared, keys);
            }
        }
        _ => {}
    }
}

/// Serializes a full fixture and asserts the keys it walks are exactly the
/// declared names (a citation through a struct shows as its `source_id`)
/// plus the allowed keys that are not graph references. A fixture that
/// leaves a declared field empty hides its key and fails here.
fn assert_walk<T: RelationOwner + serde::Serialize>(record: &T, allowed: &[&str]) {
    let declared: Vec<&str> = T::relations().iter().map(|decl| decl.name).collect();
    let mut keys = BTreeSet::new();
    reference_keys(&serde_json::to_value(record).unwrap(), &declared, &mut keys);
    let expected: BTreeSet<String> = declared
        .iter()
        .map(|name| if *name == "cites" { "source_id" } else { name })
        .chain(allowed.iter().copied())
        .map(str::to_string)
        .collect();
    assert_eq!(keys, expected, "{:?}", T::OWNER);
}

/// The shard key on every record, never a graph reference.
const SCOPE: &str = "scope_id";
/// Artifact links carry a per-entry target kind; they are walked by hand.
const LINK_TARGET: &str = "target_id";

#[test]
#[verifies("rule_prov_relation_vocabulary_closed", conformance)]
fn every_source_reference_key_is_declared_or_allowed() {
    assert_walk(
        &Source {
            schema_version: SUPPORTED_SCHEMA_VERSION,
            scope_id: ScopeId::new("default").unwrap(),
            id: sid("source_award"),
            declared_by: Some("spec".into()),
            declaration_address: Some(DeclarationAddress::new(["spec", "source", "award"]).unwrap()),
            retired: true,
            name: "Award".into(),
            source_type: SourceType::Policy,
            url: Some("https://example.test".into()),
            reference: Some("docs/award.md".into()),
            commit_pin: Some("abcdef1".into()),
            effective_date: Some(1),
            review_date: Some(2),
            supersedes: vec![sid("source_award_2019")],
            origin_thread: Some(sid("thread_a")),
            origin_message: Some(sid("message_a")),
        },
        &[SCOPE],
    );
}

#[test]
#[verifies("rule_prov_relation_vocabulary_closed", conformance)]
fn every_requirement_reference_key_is_declared_or_allowed() {
    assert_walk(
        &Requirement {
            schema_version: SUPPORTED_SCHEMA_VERSION,
            scope_id: ScopeId::new("default").unwrap(),
            id: sid("req_overtime"),
            declared_by: Some("spec".into()),
            declaration_address: Some(DeclarationAddress::new(["spec", "requirement", "overtime"]).unwrap()),
            retired: true,
            statement: "Overtime is paid".into(),
            description: Some("Description".into()),
            fog: Some("Fog".into()),
            status: RequirementStatus::Active,
            domain_id: Some(sid("domain_payroll")),
            source_refs: vec![reference("source_award")],
            refines: Some(sid("req_pay")),
            depends_on: vec![sid("req_rates")],
            supersedes: vec![sid("req_overtime_2019")],
            spawned_by: Some(sid("res_threshold")),
            origin_thread: Some(sid("thread_a")),
            origin_message: Some(sid("message_a")),
        },
        &[SCOPE],
    );
}

#[test]
#[verifies("rule_prov_relation_vocabulary_closed", conformance)]
fn every_resolution_reference_key_is_declared_or_allowed() {
    assert_walk(
        &Resolution {
            schema_version: SUPPORTED_SCHEMA_VERSION,
            scope_id: ScopeId::new("default").unwrap(),
            id: sid("res_threshold"),
            title: "Threshold".into(),
            position: "Position".into(),
            rationale: "Rationale".into(),
            status: ResolutionStatus::Approved,
            context: Some("Context".into()),
            enforcement: Some("specification".into()),
            confidence: Some(0.9),
            inputs: vec![ResolutionInput {
                input_type: ResolutionInputType::Technical,
                reference: "reference".into(),
                summary: "summary".into(),
            }],
            made_by: Some("Analyst".into()),
            approved_by: Some("Approver".into()),
            approved_at: Some(3),
            requirement_ids: vec![sid("req_overtime")],
            supersedes: vec![sid("res_threshold_draft")],
            review_on: Some("2027-01-01".into()),
            origin_thread: Some(sid("thread_a")),
            origin_message: Some(sid("message_a")),
        },
        &[SCOPE],
    );
}

#[test]
#[verifies("rule_prov_relation_vocabulary_closed", conformance)]
fn every_rule_reference_key_is_declared_or_allowed() {
    assert_walk(
        &Rule {
            schema_version: SUPPORTED_SCHEMA_VERSION,
            scope_id: ScopeId::new("default").unwrap(),
            id: sid("rule_pay"),
            declared_by: Some("spec".into()),
            declaration_address: Some(DeclarationAddress::new(["spec", "rule", "pay"]).unwrap()),
            retired: true,
            name: Some("Pay".into()),
            description: Some("Description".into()),
            statement: "Pay overtime".into(),
            status: RuleStatus::Active,
            severity: RuleSeverity::High,
            requirement_ids: vec![sid("req_overtime")],
            resolution_ids: vec![sid("res_threshold")],
            source_document: Some("docs/award.md".into()),
            source_section: Some("4.2".into()),
            origin_thread: Some(sid("thread_a")),
            origin_message: Some(sid("message_a")),
        },
        &[SCOPE],
    );
}

#[test]
#[verifies("rule_prov_relation_vocabulary_closed", conformance)]
fn every_topic_reference_key_is_declared_or_allowed() {
    assert_walk(
        &Topic {
            schema_version: SUPPORTED_SCHEMA_VERSION,
            scope_id: ScopeId::new("default").unwrap(),
            id: sid("topic_rates"),
            requirement_id: sid("req_overtime"),
            title: "Rates".into(),
            status: TopicStatus::Open,
            claimed_by: Some("agent".into()),
            claimed_at: Some(4),
            links: vec![link()],
        },
        &[SCOPE, LINK_TARGET],
    );
}

#[test]
#[verifies("rule_prov_relation_vocabulary_closed", conformance)]
fn every_question_reference_key_is_declared_or_allowed() {
    assert_walk(
        &Question {
            schema_version: SUPPORTED_SCHEMA_VERSION,
            scope_id: ScopeId::new("default").unwrap(),
            id: sid("question_threshold"),
            topic_id: sid("topic_rates"),
            requirement_id: sid("req_overtime"),
            question: "Which threshold?".into(),
            resolution_method: ResolutionMethod::Grill,
            status: QuestionStatus::Open,
            claimed_by: Some("agent".into()),
            claimed_at: Some(5),
            answer: Some("Answer".into()),
            links: vec![link()],
            resolution_id: Some(sid("res_threshold")),
            contradicts: Some(sid("req_rates")),
        },
        &[SCOPE, LINK_TARGET],
    );
}

#[test]
#[verifies("rule_prov_relation_vocabulary_closed", conformance)]
fn every_boundary_reference_key_is_declared_or_allowed() {
    assert_walk(
        &Boundary {
            schema_version: SUPPORTED_SCHEMA_VERSION,
            scope_id: ScopeId::new("default").unwrap(),
            id: sid("boundary_no_backpay"),
            requirement_id: sid("req_overtime"),
            statement: "No back pay".into(),
            source_ref: Some(reference("source_award")),
        },
        &[SCOPE],
    );
}
}
