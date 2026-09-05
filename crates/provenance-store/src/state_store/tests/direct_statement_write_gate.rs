use super::{initialized_store, seeded_requirement_store};
use crate::state_store::{CreateRequirementInput, CreateRuleInput};
use provenance_core::{RequirementStatus, RuleSeverity, RuleStatus, ScopeId, StableId};
use provenance_macros::verifies;
use std::{collections::BTreeMap, path::Path};

#[test]
#[verifies("rule_ste_direct_statement_write_gate", examples)]
fn semicolon_free_requirement_and_rule_are_written() {
    let (_dir, store, scope) = initialized_store();

    let requirement = store
        .create_requirement(requirement_input(
            &scope,
            "req_clean",
            "A clean requirement",
        ))
        .unwrap();
    let rule = store
        .create_rule(rule_input(
            &scope,
            "rule_clean",
            "A clean rule",
            &requirement.id,
        ))
        .unwrap();

    assert_eq!(rule.requirement_ids, vec![requirement.id.clone()]);
    assert_eq!(store.list_requirements(&scope).unwrap(), vec![requirement]);
    assert_eq!(store.list_rules(&scope).unwrap(), vec![rule]);
}

#[test]
#[verifies("rule_ste_direct_statement_write_gate", examples)]
fn semicolon_requirement_returns_rule_8_1_details_without_changing_canonical_state() {
    let (_dir, store, scope) = initialized_store();
    let before = canonical_state(&store.layout.state_dir());

    let error = store
        .create_requirement(requirement_input(
            &scope,
            "req_rejected",
            "First clause; second clause",
        ))
        .unwrap_err();

    assert_rule_8_1_report(&error, 12);
    assert_eq!(canonical_state(&store.layout.state_dir()), before);
}

#[test]
#[verifies("rule_ste_direct_statement_write_gate", examples)]
fn semicolon_rule_returns_rule_8_1_details_without_changing_canonical_state() {
    let (_dir, store, scope) = seeded_requirement_store();
    store
        .create_rule(rule_input(
            &scope,
            "rule_existing",
            "Existing relationship",
            &StableId::new("req_overtime").unwrap(),
        ))
        .unwrap();
    let before = canonical_state(&store.layout.state_dir());

    let error = store
        .create_rule(rule_input(
            &scope,
            "rule_rejected",
            "First clause; second clause",
            &StableId::new("req_overtime").unwrap(),
        ))
        .unwrap_err();

    assert_rule_8_1_report(&error, 12);
    assert_eq!(canonical_state(&store.layout.state_dir()), before);
}

fn requirement_input(scope: &ScopeId, id: &str, statement: &str) -> CreateRequirementInput {
    CreateRequirementInput {
        scope_id: scope.clone(),
        id: StableId::new(id).unwrap(),
        statement: statement.into(),
        description: None,
        status: RequirementStatus::Active,
        domain_id: None,
        refines: None,
        depends_on: Vec::new(),
        supersedes: Vec::new(),
        spawned_by: None,
        origin_thread: None,
        origin_message: None,
    }
}

fn rule_input(
    scope: &ScopeId,
    id: &str,
    statement: &str,
    requirement_id: &StableId,
) -> CreateRuleInput {
    CreateRuleInput {
        scope_id: scope.clone(),
        id: StableId::new(id).unwrap(),
        name: None,
        description: None,
        requirement_ids: vec![requirement_id.clone()],
        resolution_ids: Vec::new(),
        statement: statement.into(),
        status: RuleStatus::Active,
        severity: RuleSeverity::High,
        source_document: None,
        source_section: None,
        origin_thread: None,
        origin_message: None,
    }
}

fn assert_rule_8_1_report(error: &anyhow::Error, start: usize) {
    let typed = error
        .downcast_ref::<crate::state_store::statement_policy::StatementWriteError>()
        .expect("the store keeps the typed statement-write failure");
    assert_eq!(typed.report.findings[0].span.start, start);

    let report: serde_json::Value = serde_json::from_str(&error.to_string()).unwrap();
    assert_eq!(report["field"], "statement");
    assert_eq!(report["standard"], "ASD-STE100");
    assert_eq!(report["issue"], 9);
    assert_eq!(report["findings"][0]["rule"], "8.1");
    assert_eq!(report["findings"][0]["kind"], "violation");
    assert_eq!(report["findings"][0]["span"]["start"], start);
    assert_eq!(report["findings"][0]["span"]["end"], start + 1);
    assert_eq!(
        report["findings"][0]["message"],
        "Do not use semicolons in descriptive text."
    );
}

fn canonical_state(root: &camino::Utf8Path) -> BTreeMap<String, Vec<u8>> {
    let mut state = BTreeMap::new();
    collect_files(root.as_std_path(), root.as_std_path(), &mut state);
    state
}

fn collect_files(root: &Path, current: &Path, state: &mut BTreeMap<String, Vec<u8>>) {
    let mut entries = std::fs::read_dir(current)
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        if entry.file_type().unwrap().is_dir() {
            collect_files(root, &path, state);
        } else {
            state.insert(
                path.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
                std::fs::read(path).unwrap(),
            );
        }
    }
}
