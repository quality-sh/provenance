use super::initialized_store;
use crate::state_store::{CreateRequirementInput, CreateRuleInput};
use provenance_core::{RequirementStatus, RuleSeverity, RuleStatus, StableId};
use std::process::Command;

fn id(value: &str) -> StableId {
    StableId::new(value).unwrap()
}

fn git(root: &std::path::Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn create_rule(
    store: &crate::state_store::StateStore,
    scope: &provenance_core::ScopeId,
    rule_id: &str,
) -> anyhow::Result<provenance_core::Rule> {
    store.create_rule(CreateRuleInput {
        scope_id: scope.clone(),
        id: id(rule_id),
        name: None,
        description: None,
        requirement_ids: vec![id("req_anchor")],
        resolution_ids: Vec::new(),
        statement: format!("The system applies {rule_id}"),
        status: RuleStatus::Active,
        archived_in_commit: None,
        severity: RuleSeverity::Medium,
        source_document: None,
        source_section: None,
        origin_thread: None,
        origin_message: None,
    })
}

#[test]
fn native_rule_creation_accepts_an_existing_rule_after_a_commit() {
    let (dir, store, scope) = initialized_store();
    git(dir.path(), &["init", "--quiet"]);
    store
        .create_requirement(CreateRequirementInput {
            scope_id: scope.clone(),
            id: id("req_anchor"),
            statement: "The anchor requirement holds".into(),
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
    create_rule(&store, &scope, "rule_anchor").unwrap();
    git(dir.path(), &["add", "."]);
    git(
        dir.path(),
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.test",
            "commit",
            "--quiet",
            "-m",
            "Create the anchor graph",
        ],
    );

    create_rule(&store, &scope, "rule_added").unwrap();

    assert_eq!(store.list_rules(&scope).unwrap().len(), 2);
}
