use super::seeded_requirement_store;
use crate::state_store::{CreateRuleInput, MaterializeImplementationBindingInput};
use provenance_core::{RuleSeverity, RuleStatus, StableId};

#[test]
fn direct_materialization_requires_a_known_rule() {
    let (directory, store, scope) = seeded_requirement_store();
    std::fs::write(
        directory.path().join("runtime.ts"),
        "export function start() {}\n",
    )
    .unwrap();
    let error = store
        .materialize_implementation_binding(MaterializeImplementationBindingInput {
            scope_id: scope,
            rule_id: StableId::new("rule_missing").unwrap(),
            declared_by: "spec://typescript".into(),
            file: "runtime.ts".into(),
            symbol: "start".into(),
        })
        .unwrap_err();

    assert!(error.to_string().contains("does not exist"));
}

#[test]
fn repeated_materialization_updates_one_owned_primary_binding() {
    let (directory, store, scope) = seeded_requirement_store();
    std::fs::write(
        directory.path().join("runtime.ts"),
        "export function start() {}\n",
    )
    .unwrap();
    std::fs::write(
        directory.path().join("next.ts"),
        "export function begin() {}\n",
    )
    .unwrap();
    store
        .create_rule(CreateRuleInput {
            scope_id: scope.clone(),
            id: StableId::new("rule_start").unwrap(),
            name: None,
            description: None,
            requirement_ids: vec![StableId::new("req_overtime").unwrap()],
            resolution_ids: Vec::new(),
            statement: "Workflows start".into(),
            status: RuleStatus::Active,
            severity: RuleSeverity::Medium,
            source_document: None,
            source_section: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
    let input = |file: &str, symbol: &str| MaterializeImplementationBindingInput {
        scope_id: scope.clone(),
        rule_id: StableId::new("rule_start").unwrap(),
        declared_by: "spec://typescript".into(),
        file: file.into(),
        symbol: symbol.into(),
    };

    let first = store
        .materialize_implementation_binding(input("runtime.ts", "start"))
        .unwrap();
    let second = store
        .materialize_implementation_binding(input("next.ts", "begin"))
        .unwrap();

    assert_eq!(first.id, second.id);
    let stored = store.list_implementation_bindings(&scope).unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].file, camino::Utf8PathBuf::from("next.ts"));
}

#[test]
fn active_binding_view_excludes_retired_history() {
    let (directory, store, scope) = seeded_requirement_store();
    std::fs::write(
        directory.path().join("runtime.ts"),
        "export function start() {}\n",
    )
    .unwrap();
    store
        .create_rule(CreateRuleInput {
            scope_id: scope.clone(),
            id: StableId::new("rule_start").unwrap(),
            name: None,
            description: None,
            requirement_ids: vec![StableId::new("req_overtime").unwrap()],
            resolution_ids: Vec::new(),
            statement: "Workflows start".into(),
            status: RuleStatus::Active,
            severity: RuleSeverity::Medium,
            source_document: None,
            source_section: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
    store
        .materialize_implementation_binding(MaterializeImplementationBindingInput {
            scope_id: scope.clone(),
            rule_id: StableId::new("rule_start").unwrap(),
            declared_by: "spec://typescript".into(),
            file: "runtime.ts".into(),
            symbol: "start".into(),
        })
        .unwrap();
    let path = crate::shards::implementation_bindings_path(&store.layout, &scope);
    store
        .mutate_jsonl_records(
            &path,
            |records: &mut Vec<provenance_core::ImplementationBinding>| {
                records[0].retired = true;
                Ok(())
            },
        )
        .unwrap();

    assert_eq!(store.list_implementation_bindings(&scope).unwrap().len(), 1);
    assert!(store
        .active_implementation_bindings(&scope)
        .unwrap()
        .is_empty());
}

#[test]
fn direct_materialization_requires_a_nonempty_owner() {
    let (directory, store, scope) = seeded_requirement_store();
    std::fs::write(
        directory.path().join("runtime.ts"),
        "export function start() {}\n",
    )
    .unwrap();
    store
        .create_rule(CreateRuleInput {
            scope_id: scope.clone(),
            id: StableId::new("rule_start").unwrap(),
            name: None,
            description: None,
            requirement_ids: vec![StableId::new("req_overtime").unwrap()],
            resolution_ids: Vec::new(),
            statement: "Workflows start".into(),
            status: RuleStatus::Active,
            severity: RuleSeverity::Medium,
            source_document: None,
            source_section: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();

    let error = store
        .materialize_implementation_binding(MaterializeImplementationBindingInput {
            scope_id: scope,
            rule_id: StableId::new("rule_start").unwrap(),
            declared_by: " ".into(),
            file: "runtime.ts".into(),
            symbol: "start".into(),
        })
        .unwrap_err();

    assert!(error.to_string().contains("declared_by must not be empty"));
}

#[test]
fn direct_materialization_rejects_platform_specific_path_separators() {
    let (_directory, store, scope) = seeded_requirement_store();
    store
        .create_rule(CreateRuleInput {
            scope_id: scope.clone(),
            id: StableId::new("rule_start").unwrap(),
            name: None,
            description: None,
            requirement_ids: vec![StableId::new("req_overtime").unwrap()],
            resolution_ids: Vec::new(),
            statement: "Workflows start".into(),
            status: RuleStatus::Active,
            severity: RuleSeverity::Medium,
            source_document: None,
            source_section: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();

    let error = store
        .materialize_implementation_binding(MaterializeImplementationBindingInput {
            scope_id: scope,
            rule_id: StableId::new("rule_start").unwrap(),
            declared_by: "spec://typescript".into(),
            file: r"src\runtime.ts".into(),
            symbol: "start".into(),
        })
        .unwrap_err();

    assert!(error.to_string().contains("repository-relative"));
}

#[test]
fn another_owner_cannot_replace_the_primary_binding() {
    let (directory, store, scope) = seeded_requirement_store();
    std::fs::write(
        directory.path().join("runtime.ts"),
        "export function start() {}\n",
    )
    .unwrap();
    store
        .create_rule(CreateRuleInput {
            scope_id: scope.clone(),
            id: StableId::new("rule_start").unwrap(),
            name: None,
            description: None,
            requirement_ids: vec![StableId::new("req_overtime").unwrap()],
            resolution_ids: Vec::new(),
            statement: "Workflows start".into(),
            status: RuleStatus::Active,
            severity: RuleSeverity::Medium,
            source_document: None,
            source_section: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
    let input = |owner: &str| MaterializeImplementationBindingInput {
        scope_id: scope.clone(),
        rule_id: StableId::new("rule_start").unwrap(),
        declared_by: owner.into(),
        file: "runtime.ts".into(),
        symbol: "start".into(),
    };
    store
        .materialize_implementation_binding(input("spec://typescript/first"))
        .unwrap();

    let error = store
        .materialize_implementation_binding(input("spec://typescript/second"))
        .unwrap_err();

    assert!(error
        .to_string()
        .contains("owned by another declaration owner"));
}
