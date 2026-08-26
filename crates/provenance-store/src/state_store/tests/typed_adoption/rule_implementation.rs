use super::*;

fn seeded_rule_with_implementation() -> (tempfile::TempDir, StateStore, ScopeId) {
    let (dir, store, scope) = initialized_store();
    store
        .apply_typed_spec(
            &scope,
            document(
                OWNER,
                vec![requirement(
                    "canonical",
                    Some("req_owned"),
                    "The canonical Requirement is stable",
                )],
                Vec::new(),
            ),
        )
        .unwrap();
    store
        .create_rule(CreateRuleInput {
            scope_id: scope.clone(),
            id: StableId::new("rule_existing").unwrap(),
            name: None,
            description: None,
            requirement_id: Some(StableId::new("req_owned").unwrap()),
            resolution_id: None,
            statement: "The canonical Rule keeps its identity".to_string(),
            status: RuleStatus::Active,
            severity: RuleSeverity::Medium,
            source_document: None,
            source_section: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
    let source_dir = store.layout.root().join("src");
    std::fs::create_dir_all(&source_dir).unwrap();
    std::fs::write(source_dir.join("enforcement.rs"), "fn enforce() {}\n").unwrap();
    store
        .materialize_implementation_binding(MaterializeImplementationBindingInput {
            scope_id: scope.clone(),
            rule_id: StableId::new("rule_existing").unwrap(),
            declared_by: OWNER.to_string(),
            file: camino::Utf8PathBuf::from("src/enforcement.rs"),
            symbol: "enforce".to_string(),
        })
        .unwrap();
    let path = crate::shards::implementation_bindings_path(&store.layout, &scope);
    store
        .mutate_jsonl_records(
            &path,
            |records: &mut Vec<provenance_core::ImplementationBinding>| {
                records[0].id = StableId::new("implementation_binding_imported").unwrap();
                Ok(())
            },
        )
        .unwrap();
    (dir, store, scope)
}

fn adoption_input(implementation: Option<TypedImplementationInput>) -> TypedSpecInput {
    let mut input = document(
        OWNER,
        vec![requirement(
            "canonical",
            Some("req_owned"),
            "The canonical Requirement is stable",
        )],
        vec![target(TypedDeclarationKind::Rule, "rule_existing")],
    );
    input.rules.push(TypedRuleInput {
        key: "enforcement".to_string(),
        id: Some("rule_existing".to_string()),
        address: None,
        requirement: None,
        requirements: vec!["canonical".to_string()],
        statement: "The canonical Rule keeps its identity".to_string(),
        name: None,
        description: None,
        implementation,
    });
    input
}

#[test]
fn rule_adoption_preserves_an_exact_or_omitted_existing_implementation() {
    let (_dir, store, scope) = seeded_rule_with_implementation();
    let exact = adoption_input(Some(TypedImplementationInput {
        file: camino::Utf8PathBuf::from("src/enforcement.rs"),
        symbol: "enforce".to_string(),
    }));

    let plan = store.plan_typed_spec(&scope, exact.clone()).unwrap();
    assert_eq!(plan.conflicts, 0);
    assert!(plan.resources.iter().all(|resource| resource
        .changes
        .iter()
        .all(|change| change.field != "implementation")));
    store.apply_typed_spec(&scope, exact).unwrap();

    let omitted = adoption_input(None);
    let replay = store.plan_typed_spec(&scope, omitted.clone()).unwrap();
    assert_eq!(replay.conflicts, 0);
    assert_eq!(replay.implementation_bindings.len(), 1);
    store.apply_typed_spec(&scope, omitted).unwrap();
    let bindings = store.list_implementation_bindings(&scope).unwrap();
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].id.as_str(), "implementation_binding_imported");
    assert!(!bindings[0].retired);
    assert_eq!(bindings[0].file.as_str(), "src/enforcement.rs");
    assert_eq!(bindings[0].symbol, "enforce");
}
