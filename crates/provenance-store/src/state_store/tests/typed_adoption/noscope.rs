use super::*;

#[test]
fn noscope_sized_source_and_requirement_adoption_preserves_all_ids_and_edges() {
    let (_dir, store, scope) = initialized_store();
    create_unowned_source(&store, &scope, "Policy");
    store
        .create_rule(CreateRuleInput {
            scope_id: scope.clone(),
            id: StableId::new("rule_unrelated").unwrap(),
            name: None,
            description: None,
            requirement_id: None,
            resolution_id: None,
            statement: "The unrelated Rule stays unowned".to_string(),
            status: RuleStatus::Active,
            severity: RuleSeverity::Medium,
            source_document: None,
            source_section: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();

    let mut requirements = Vec::new();
    let mut targets = vec![target(TypedDeclarationKind::Source, "source_policy")];
    for index in 0..70 {
        let id = format!("req_noscope_{index:02}");
        let statement = format!("Canonical Requirement number {index} stays stable");
        create_unowned_requirement(&store, &scope, &id, &statement);
        store
            .add_source_reference(AddSourceReferenceInput {
                scope_id: scope.clone(),
                source_id: StableId::new("source_policy").unwrap(),
                requirement_id: StableId::new(&id).unwrap(),
                clause: None,
            })
            .unwrap();
        let mut declaration = requirement(&format!("canonical-{index:02}"), Some(&id), &statement);
        declaration.sources.push("policy".to_string());
        requirements.push(declaration);
        targets.push(target(TypedDeclarationKind::Requirement, &id));
    }
    let edges_path = crate::shards::edges_path(&store.layout);
    store
        .mutate_jsonl_records(&edges_path, |edges: &mut Vec<provenance_core::Edge>| {
            edges[0].id = StableId::new("edge_imported_identity").unwrap();
            Ok(())
        })
        .unwrap();
    let input = TypedSpecInput {
        schema_version: SUPPORTED_SCHEMA_VERSION.0,
        spec: "migration".to_string(),
        declared_by: OWNER.to_string(),
        adopt_unowned: targets,
        sources: vec![TypedSourceInput {
            key: "policy".to_string(),
            id: Some("source_policy".to_string()),
            name: "Policy".to_string(),
            kind: "document".to_string(),
            url: None,
            reference: Some("docs/policy.md".to_string()),
        }],
        requirements,
        rules: Vec::new(),
    };

    let plan = store.plan_typed_spec(&scope, input.clone()).unwrap();
    assert_eq!((plan.created, plan.conflicts), (0, 0));
    assert_eq!(plan.resources.len(), 71);
    store.apply_typed_spec(&scope, input.clone()).unwrap();
    assert_eq!(store.list_requirements(&scope).unwrap().len(), 70);
    assert_eq!(store.list_sources(&scope).unwrap().len(), 1);
    let edges = store.list_edges().unwrap();
    assert_eq!(edges.len(), 70);
    assert!(edges
        .iter()
        .any(|edge| edge.id.as_str() == "edge_imported_identity"));
    let unrelated = &store.list_rules(&scope).unwrap()[0];
    assert_eq!(unrelated.id.as_str(), "rule_unrelated");
    assert_eq!(unrelated.declared_by, None);
    let replay = store.plan_typed_spec(&scope, input).unwrap();
    assert!(replay
        .resources
        .iter()
        .all(|resource| resource.state == ReconcileState::Unchanged));
}
