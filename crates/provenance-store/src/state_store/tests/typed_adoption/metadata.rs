use super::*;

fn seed_external_metadata(store: &StateStore, scope: &ScopeId) {
    store
        .create_source(CreateSourceInput {
            scope_id: scope.clone(),
            id: StableId::new("source_metadata").unwrap(),
            name: "Policy".to_string(),
            source_type: SourceType::Document,
            url: Some("https://example.test/policy".to_string()),
            reference: Some("docs/policy.md".to_string()),
            commit_pin: Some("0123456789abcdef0123456789abcdef01234567".to_string()),
            effective_date: Some(1_700_000_000_000),
            review_date: None,
            supersedes: Vec::new(),
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
    store
        .create_requirement(CreateRequirementInput {
            scope_id: scope.clone(),
            id: StableId::new("req_metadata").unwrap(),
            statement: STATEMENT.to_string(),
            description: Some("Metadata authored outside the typed spec".to_string()),
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
    store
        .add_source_reference(AddSourceReferenceInput {
            scope_id: scope.clone(),
            source_id: StableId::new("source_metadata").unwrap(),
            requirement_id: StableId::new("req_metadata").unwrap(),
            clause: Some("section 4".to_string()),
        })
        .unwrap();
    store
        .create_rule(CreateRuleInput {
            scope_id: scope.clone(),
            id: StableId::new("rule_metadata").unwrap(),
            name: None,
            description: None,
            requirement_ids: vec![StableId::new("req_metadata").unwrap()],
            resolution_ids: Vec::new(),
            statement: "The canonical Rule keeps its identity".to_string(),
            status: RuleStatus::Active,
            severity: RuleSeverity::High,
            source_document: Some("docs/policy.md".to_string()),
            source_section: Some("Enforcement".to_string()),
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
}

fn adoption_input() -> TypedSpecInput {
    TypedSpecInput {
        schema_version: SUPPORTED_SCHEMA_VERSION.0,
        spec: "migration".to_string(),
        declared_by: OWNER.to_string(),
        adopt_unowned: vec![
            target(TypedDeclarationKind::Source, "source_metadata"),
            target(TypedDeclarationKind::Requirement, "req_metadata"),
            target(TypedDeclarationKind::Rule, "rule_metadata"),
        ],
        sources: vec![TypedSourceInput {
            supersedes: None,

            key: "policy".to_string(),
            id: Some("source_metadata".to_string()),
            name: "Policy".to_string(),
            kind: "document".to_string(),
            url: None,
            reference: Some("docs/policy.md".to_string()),
        }],
        requirements: vec![TypedRequirementInput {
            refines: None,
            depends_on: None,
            supersedes: None,
            spawned_by: None,

            key: "canonical".to_string(),
            id: Some("req_metadata".to_string()),
            statement: STATEMENT.to_string(),
            description: None,
            sources: vec!["policy".to_string()],
        }],
        rules: vec![TypedRuleInput {
            resolution_ids: None,

            key: "enforcement".to_string(),
            id: Some("rule_metadata".to_string()),
            address: None,
            requirement: None,
            requirements: vec!["canonical".to_string()],
            statement: "The canonical Rule keeps its identity".to_string(),
            name: None,
            description: None,
            implementation: None,
        }],
    }
}

#[test]
fn adoption_preserves_metadata_outside_the_typed_declaration_surface() {
    let (_dir, store, scope) = initialized_store();
    seed_external_metadata(&store, &scope);
    let input = adoption_input();

    let plan = store.plan_typed_spec(&scope, input.clone()).unwrap();
    assert_eq!((plan.created, plan.conflicts), (0, 0));
    store.apply_typed_spec(&scope, input).unwrap();

    let source = &store.list_sources(&scope).unwrap()[0];
    assert_eq!(
        source.commit_pin.as_deref(),
        Some("0123456789abcdef0123456789abcdef01234567")
    );
    assert_eq!(source.url.as_deref(), Some("https://example.test/policy"));
    assert_eq!(source.effective_date, Some(1_700_000_000_000));
    let requirement = &store.list_requirements(&scope).unwrap()[0];
    assert_eq!(
        requirement.description.as_deref(),
        Some("Metadata authored outside the typed spec")
    );
    assert_eq!(
        requirement.source_refs[0].clause.as_deref(),
        Some("section 4")
    );
    let rule = &store.list_rules(&scope).unwrap()[0];
    assert_eq!(rule.severity, RuleSeverity::High);
    assert_eq!(rule.source_document.as_deref(), Some("docs/policy.md"));
    assert_eq!(rule.source_section.as_deref(), Some("Enforcement"));
}
