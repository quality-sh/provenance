use super::{seeded_requirement_store, seeded_source_requirement_store};
use crate::state_store::{CreateRequirementInput, CreateResolutionInput, CreateRuleInput};
use provenance_core::{RequirementStatus, ResolutionStatus, RuleSeverity, RuleStatus, StableId};

fn sid(value: &str) -> StableId {
    StableId::new(value).unwrap()
}

fn requirement(store: &crate::state_store::StateStore, scope: &provenance_core::ScopeId, id: &str) {
    store
        .create_requirement(CreateRequirementInput {
            scope_id: scope.clone(),
            id: sid(id),
            statement: format!("{id} statement"),
            description: None,
            status: RequirementStatus::Active,
            domain_id: None,
            refines: None,
            depends_on: Vec::new(),
            supersedes: Vec::new(),
            spawned_by: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
}

#[test]
fn a_refines_chain_back_to_the_owner_is_refused() {
    let (_dir, store, scope) = seeded_requirement_store();
    requirement(&store, &scope, "req_leave");
    requirement(&store, &scope, "req_rates");
    store
        .set_requirement_refines(&scope, &sid("req_leave"), sid("req_overtime"))
        .unwrap();
    store
        .set_requirement_refines(&scope, &sid("req_rates"), sid("req_leave"))
        .unwrap();

    let error = store
        .set_requirement_refines(&scope, &sid("req_overtime"), sid("req_rates"))
        .unwrap_err();

    assert_eq!(
        error.to_string(),
        "refines from req_overtime to req_rates would form a cycle"
    );
    let error = store
        .add_requirement_depends_on(&scope, &sid("req_overtime"), sid("req_overtime"))
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "depends_on from req_overtime to req_overtime would form a cycle"
    );
}

#[test]
fn a_missing_target_is_refused_by_kind() {
    let (_dir, store, scope) = seeded_requirement_store();
    let error = store
        .set_requirement_spawned_by(&scope, &sid("req_overtime"), sid("res_missing"))
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "resolution res_missing does not exist (--target-id)"
    );
    let error = store
        .add_requirement_supersedes(&scope, &sid("req_overtime"), sid("req_missing"))
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "requirement req_missing does not exist (--target-id)"
    );
}

#[test]
fn a_missing_owner_is_refused_by_its_own_id_and_flag() {
    let (_dir, store, scope) = seeded_source_requirement_store();
    let error = store
        .set_requirement_refines(&scope, &sid("req_ghost"), sid("req_overtime"))
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "requirement req_ghost does not exist (--requirement-id)"
    );
    let error = store
        .clear_requirement_refines(&scope, &sid("req_ghost"))
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "requirement req_ghost does not exist (--requirement-id)"
    );
    let error = store
        .clear_requirement_depends_on(&scope, &sid("req_ghost"), &sid("req_overtime"))
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "requirement req_ghost does not exist (--requirement-id)"
    );
    let error = store
        .add_rule_requirement(&scope, &sid("rule_ghost"), sid("req_overtime"))
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "rule rule_ghost does not exist (--rule-id)"
    );
    let error = store
        .clear_rule_requirement(&scope, &sid("rule_ghost"), &sid("req_overtime"))
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "rule rule_ghost does not exist (--rule-id)"
    );
    let error = store
        .add_resolution_requirement(&scope, &sid("res_ghost"), sid("req_overtime"))
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "resolution res_ghost does not exist (--resolution-id)"
    );
    let error = store
        .add_source_supersedes(&scope, &sid("source_ghost"), sid("source_schads"))
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "source source_ghost does not exist (--source-id)"
    );
    let error = store
        .set_question_contradicts(&scope, &sid("question_ghost"), sid("req_overtime"))
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "question question_ghost does not exist (--id)"
    );
    let error = store
        .clear_source_reference(&scope, &sid("req_ghost"), &sid("source_schads"))
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "requirement req_ghost does not exist (--requirement-id)"
    );
    let error = store
        .add_source_reference(crate::state_store::AddSourceReferenceInput {
            scope_id: scope.clone(),
            source_id: sid("source_schads"),
            requirement_id: sid("req_ghost"),
            clause: None,
        })
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "requirement req_ghost does not exist (--requirement-id)"
    );
    let error = store
        .add_source_reference(crate::state_store::AddSourceReferenceInput {
            scope_id: scope,
            source_id: sid("source_ghost"),
            requirement_id: sid("req_overtime"),
            clause: None,
        })
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "source source_ghost does not exist (--target-id)"
    );
}

#[test]
fn a_clear_refusal_names_the_relation_it_searched() {
    let (_dir, store, scope) = seeded_requirement_store();
    requirement(&store, &scope, "req_rates");
    requirement(&store, &scope, "req_leave");
    store
        .add_requirement_depends_on(&scope, &sid("req_overtime"), sid("req_rates"))
        .unwrap();

    let error = store
        .clear_requirement_supersedes(&scope, &sid("req_overtime"), &sid("req_rates"))
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "requirement req_overtime does not name requirement req_rates under supersedes"
    );
    let error = store
        .clear_requirement_depends_on(&scope, &sid("req_overtime"), &sid("req_leave"))
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "requirement req_overtime does not name requirement req_leave under depends_on"
    );
}

#[test]
fn a_required_list_keeps_its_last_entry() {
    let (_dir, store, scope) = seeded_requirement_store();
    store
        .create_resolution(CreateResolutionInput {
            scope_id: scope.clone(),
            id: sid("res_overtime"),
            title: "Overtime".into(),
            requirement_ids: vec![sid("req_overtime")],
            supersedes: Vec::new(),
            position: "Pay it".into(),
            rationale: "The award says so".into(),
            status: ResolutionStatus::Proposed,
            context: None,
            enforcement: None,
            confidence: None,
            inputs: Vec::new(),
            made_by: None,
            approved_by: None,
            approved_at: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
    store
        .create_rule(CreateRuleInput {
            scope_id: scope.clone(),
            id: sid("rule_pay"),
            name: None,
            description: None,
            requirement_ids: vec![sid("req_overtime")],
            resolution_ids: vec![sid("res_overtime")],
            statement: "Pay overtime after the threshold".into(),
            status: RuleStatus::Active,
            severity: RuleSeverity::High,
            source_document: None,
            source_section: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();

    let error = store
        .clear_rule_requirement(&scope, &sid("rule_pay"), &sid("req_overtime"))
        .unwrap_err();
    assert_eq!(error.to_string(), "a rule needs one requirement");
    let error = store
        .clear_resolution_requirement(&scope, &sid("res_overtime"), &sid("req_overtime"))
        .unwrap_err();
    assert_eq!(error.to_string(), "a resolution needs one requirement");

    let rule = store
        .clear_rule_resolution(&scope, &sid("rule_pay"), &sid("res_overtime"))
        .unwrap();
    assert!(rule.resolution_ids.is_empty(), "an optional list empties");
}

#[test]
fn a_create_with_no_requirement_is_refused() {
    let (_dir, store, scope) = seeded_requirement_store();
    let error = store
        .create_rule(CreateRuleInput {
            scope_id: scope,
            id: sid("rule_orphan"),
            name: None,
            description: None,
            requirement_ids: Vec::new(),
            resolution_ids: Vec::new(),
            statement: "An orphan".into(),
            status: RuleStatus::Active,
            severity: RuleSeverity::Low,
            source_document: None,
            source_section: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap_err();
    assert_eq!(error.to_string(), "a rule needs one requirement");
}

#[test]
fn a_citation_clears_by_source() {
    let (_dir, store, scope) = seeded_source_requirement_store();
    store
        .add_source_reference(crate::state_store::AddSourceReferenceInput {
            scope_id: scope.clone(),
            source_id: sid("source_schads"),
            requirement_id: sid("req_overtime"),
            clause: Some("4.2".into()),
        })
        .unwrap();
    let requirement = store
        .clear_source_reference(&scope, &sid("req_overtime"), &sid("source_schads"))
        .unwrap();
    assert!(requirement.source_refs.is_empty());
    let error = store
        .clear_source_reference(&scope, &sid("req_overtime"), &sid("source_schads"))
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "requirement req_overtime does not name source source_schads under cites"
    );
}
