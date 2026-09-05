use super::*;

fn rule_input(requirement_key: &str, statement: &str) -> TypedSpecInput {
    let mut input = document(
        OWNER,
        vec![
            requirement(
                "first",
                Some("req_first"),
                "The first Requirement is stable",
            ),
            requirement(
                "second",
                Some("req_second"),
                "The second Requirement is stable",
            ),
        ],
        vec![target(TypedDeclarationKind::Rule, "rule_existing")],
    );
    input.rules.push(TypedRuleInput {
        resolution_ids: None,

        key: "enforcement".to_string(),
        id: Some("rule_existing".to_string()),
        address: None,
        requirement: None,
        requirements: vec![requirement_key.to_string()],
        statement: statement.to_string(),
        name: None,
        description: None,
        implementation: None,
    });
    input
}

fn store_with_unowned_rule() -> (tempfile::TempDir, StateStore, ScopeId) {
    let (dir, store, scope) = initialized_store();
    store
        .apply_typed_spec(
            &scope,
            document(
                OWNER,
                vec![
                    requirement(
                        "first",
                        Some("req_first"),
                        "The first Requirement is stable",
                    ),
                    requirement(
                        "second",
                        Some("req_second"),
                        "The second Requirement is stable",
                    ),
                ],
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
            requirement_ids: vec![StableId::new("req_first").unwrap()],
            resolution_ids: Vec::new(),
            statement: "The canonical Rule keeps its identity".to_string(),
            status: RuleStatus::Active,
            severity: RuleSeverity::Medium,
            source_document: None,
            source_section: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
    (dir, store, scope)
}

#[test]
fn rule_adoption_rejects_a_definition_change() {
    let (_dir, store, scope) = store_with_unowned_rule();
    let input = rule_input("first", "A changed Rule statement");

    let plan = store.plan_typed_spec(&scope, input.clone()).unwrap();
    assert_eq!(plan.conflicts, 1);
    assert!(plan.resources.iter().any(|resource| resource
        .changes
        .iter()
        .any(|change| change.field == "statement")));
    assert!(store.apply_typed_spec(&scope, input).is_err());
}

#[test]
fn rule_adoption_rejects_a_requirement_relationship_change() {
    let (_dir, store, scope) = store_with_unowned_rule();
    let before = store.list_rules(&scope).unwrap();
    let input = rule_input("second", "The canonical Rule keeps its identity");

    let plan = store.plan_typed_spec(&scope, input.clone()).unwrap();
    assert_eq!(plan.conflicts, 1);
    assert!(plan.resources.iter().any(|resource| resource
        .changes
        .iter()
        .any(|change| change.field == "relationships")));
    assert!(store.apply_typed_spec(&scope, input).is_err());
    assert_eq!(store.list_rules(&scope).unwrap(), before);
}

#[test]
#[allow(clippy::too_many_lines)]
fn rule_adoption_preserves_a_resolution_relationship_outside_the_typed_surface() {
    let (_dir, store, scope) = initialized_store();
    store
        .apply_typed_spec(
            &scope,
            document(
                OWNER,
                vec![
                    requirement(
                        "first",
                        Some("req_first"),
                        "The first Requirement is stable",
                    ),
                    requirement(
                        "second",
                        Some("req_second"),
                        "The second Requirement is stable",
                    ),
                ],
                Vec::new(),
            ),
        )
        .unwrap();
    store
        .create_resolution(CreateResolutionInput {
            scope_id: scope.clone(),
            id: StableId::new("req_first").unwrap(),
            title: "Existing decision".to_string(),
            requirement_ids: vec![StableId::new("req_second").unwrap()],
            supersedes: Vec::new(),
            position: "Keep the canonical Rule".to_string(),
            rationale: "The existing decision remains canonical".to_string(),
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
            id: StableId::new("rule_existing").unwrap(),
            name: None,
            description: None,
            requirement_ids: vec![StableId::new("req_second").unwrap()],
            resolution_ids: vec![StableId::new("req_first").unwrap()],
            statement: "The canonical Rule keeps its identity".to_string(),
            status: RuleStatus::Active,
            severity: RuleSeverity::Medium,
            source_document: None,
            source_section: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
    let before = store.list_rules(&scope).unwrap();
    let mut input = document(
        OWNER,
        vec![
            requirement(
                "first",
                Some("req_first"),
                "The first Requirement is stable",
            ),
            requirement(
                "second",
                Some("req_second"),
                "The second Requirement is stable",
            ),
        ],
        vec![target(TypedDeclarationKind::Rule, "rule_existing")],
    );
    input.rules.push(TypedRuleInput {
        resolution_ids: None,

        key: "enforcement".to_string(),
        id: Some("rule_existing".to_string()),
        address: None,
        requirement: None,
        requirements: vec!["second".to_string()],
        statement: "The canonical Rule keeps its identity".to_string(),
        name: None,
        description: None,
        implementation: None,
    });

    let plan = store.plan_typed_spec(&scope, input.clone()).unwrap();
    assert_eq!(plan.conflicts, 0);
    store.apply_typed_spec(&scope, input).unwrap();
    let rules = store.list_rules(&scope).unwrap();
    assert_eq!(rules.len(), before.len());
    let existing = rules
        .iter()
        .find(|rule| rule.id.as_str() == "rule_existing")
        .unwrap();
    assert!(existing
        .resolution_ids
        .iter()
        .any(|id| id.as_str() == "req_first"));
}
