use super::initialized_store;
use crate::state_store::{
    AddSourceReferenceInput, CreateRequirementInput, CreateResolutionInput, CreateRuleInput,
    CreateSourceInput, MaterializeImplementationBindingInput, ReconcileState, StateStore,
};
use provenance_core::protocol::{
    TypedAdoptionTarget, TypedDeclarationKind, TypedImplementationInput, TypedRequirementInput,
    TypedRuleInput, TypedSourceInput, TypedSpecInput,
};
use provenance_core::{
    RequirementStatus, ResolutionStatus, RuleSeverity, RuleStatus, ScopeId, SourceType, StableId,
    SUPPORTED_SCHEMA_VERSION,
};

mod metadata;
mod noscope;
mod rule_implementation;
mod rule_mismatches;
mod source_kind;

const OWNER: &str = "spec://rust/migration";
const STATEMENT: &str = "The canonical Requirement keeps its identity";

fn target(kind: TypedDeclarationKind, id: &str) -> TypedAdoptionTarget {
    TypedAdoptionTarget {
        kind,
        id: id.to_string(),
    }
}

fn requirement(key: &str, id: Option<&str>, statement: &str) -> TypedRequirementInput {
    TypedRequirementInput {
        key: key.to_string(),
        id: id.map(str::to_string),
        statement: statement.to_string(),
        description: None,
        sources: Vec::new(),
    }
}

fn document(
    owner: &str,
    requirements: Vec<TypedRequirementInput>,
    adopt_unowned: Vec<TypedAdoptionTarget>,
) -> TypedSpecInput {
    TypedSpecInput {
        actor: None,
        schema_version: SUPPORTED_SCHEMA_VERSION.0,
        spec: "migration".to_string(),
        declared_by: owner.to_string(),
        adopt_unowned,
        sources: Vec::new(),
        requirements,
        rules: Vec::new(),
    }
}

fn create_unowned_requirement(store: &StateStore, scope: &ScopeId, id: &str, statement: &str) {
    store
        .create_requirement(
            None,
            CreateRequirementInput {
                scope_id: scope.clone(),
                id: StableId::new(id).unwrap(),
                statement: statement.to_string(),
                description: None,
                status: RequirementStatus::Active,
                domain_id: None,
                origin_thread: None,
                origin_message: None,
            },
        )
        .unwrap();
}

fn create_unowned_source(store: &StateStore, scope: &ScopeId, name: &str) {
    store
        .create_source(
            None,
            CreateSourceInput {
                scope_id: scope.clone(),
                id: StableId::new("source_policy").unwrap(),
                name: name.to_string(),
                source_type: SourceType::Document,
                url: None,
                reference: Some("docs/policy.md".to_string()),
                commit_pin: None,
                effective_date: None,
                review_date: None,
                superseded_by: None,
                origin_thread: None,
                origin_message: None,
            },
        )
        .unwrap();
}

#[test]
fn explicit_id_without_adoption_keeps_the_default_conflict_and_bytes() {
    let (_dir, store, scope) = initialized_store();
    create_unowned_requirement(&store, &scope, "req_existing", STATEMENT);
    let input = document(
        OWNER,
        vec![requirement("canonical", Some("req_existing"), STATEMENT)],
        Vec::new(),
    );
    let path = crate::shards::requirements_path(&store.layout, &scope);
    let before = std::fs::read(&path).unwrap();

    let plan = store.plan_typed_spec(&scope, input.clone()).unwrap();
    assert_eq!(plan.conflicts, 1);
    assert_eq!(plan.resources[0].changes.len(), 1);
    assert_eq!(plan.resources[0].changes[0].field, "declared_by");
    assert_eq!(plan.resources[0].changes[0].before, "unowned");
    assert_eq!(plan.resources[0].changes[0].after, OWNER);
    assert!(store.apply_typed_spec(None, &scope, input).is_err());
    assert_eq!(std::fs::read(path).unwrap(), before);
}

#[test]
fn exact_requirement_adoption_changes_only_owner_and_address_then_replays_unchanged() {
    let (_dir, store, scope) = initialized_store();
    create_unowned_requirement(&store, &scope, "req_existing", STATEMENT);
    let input = document(
        OWNER,
        vec![requirement("canonical", Some("req_existing"), STATEMENT)],
        vec![target(TypedDeclarationKind::Requirement, "req_existing")],
    );

    let plan = store.plan_typed_spec(&scope, input.clone()).unwrap();
    assert_eq!((plan.created, plan.conflicts), (0, 0));
    assert_eq!(plan.resources[0].id.as_str(), "req_existing");
    let mut fields = plan.resources[0]
        .changes
        .iter()
        .map(|change| change.field.as_str())
        .collect::<Vec<_>>();
    fields.sort_unstable();
    assert_eq!(fields, ["address", "declared_by"]);

    let applied = store.apply_typed_spec(None, &scope, input.clone()).unwrap();
    assert_eq!(
        (applied.created, applied.conflicts, applied.moved),
        (0, 0, 1)
    );
    let record = &store.list_requirements(&scope).unwrap()[0];
    assert_eq!(record.id.as_str(), "req_existing");
    assert_eq!(record.statement, STATEMENT);
    assert_eq!(record.declared_by.as_deref(), Some(OWNER));
    assert_eq!(
        record.declaration_address.as_ref().unwrap().segments(),
        ["migration", "requirement", "canonical"]
    );

    let replay = store.plan_typed_spec(&scope, input).unwrap();
    assert_eq!((replay.created, replay.updated, replay.moved), (0, 0, 0));
    assert_eq!(replay.unchanged, 1);
}

#[test]
fn adoption_never_transfers_a_declaration_between_owners() {
    let (_dir, store, scope) = initialized_store();
    let owner_a = document(
        "spec://owner/a",
        vec![requirement("canonical", Some("req_existing"), STATEMENT)],
        Vec::new(),
    );
    store.apply_typed_spec(None, &scope, owner_a).unwrap();
    let before = store.list_requirements(&scope).unwrap();
    let owner_b = document(
        "spec://owner/b",
        vec![requirement("canonical", Some("req_existing"), STATEMENT)],
        vec![target(TypedDeclarationKind::Requirement, "req_existing")],
    );

    let plan = store.plan_typed_spec(&scope, owner_b.clone()).unwrap();
    assert_eq!(plan.conflicts, 1);
    assert!(store.apply_typed_spec(None, &scope, owner_b).is_err());
    assert_eq!(store.list_requirements(&scope).unwrap(), before);
}

#[test]
fn invalid_adoption_targets_are_rejected_before_reconciliation() {
    let cases = [
        (
            document(
                OWNER,
                Vec::new(),
                vec![target(TypedDeclarationKind::Requirement, "req_existing")],
            ),
            "does not name a declaration",
        ),
        (
            document(
                OWNER,
                vec![requirement("canonical", None, STATEMENT)],
                vec![target(TypedDeclarationKind::Requirement, "req_existing")],
            ),
            "same explicit id",
        ),
        (
            document(
                OWNER,
                vec![requirement("canonical", Some("req_existing"), STATEMENT)],
                vec![
                    target(TypedDeclarationKind::Requirement, "req_existing"),
                    target(TypedDeclarationKind::Requirement, "req_existing"),
                ],
            ),
            "duplicate adoption target",
        ),
        (
            document(
                OWNER,
                vec![requirement("canonical", Some("req_existing"), STATEMENT)],
                vec![target(TypedDeclarationKind::Requirement, "Bad Id")],
            ),
            "must use lowercase ASCII",
        ),
        (
            document(
                OWNER,
                vec![requirement("canonical", Some("req_typo"), STATEMENT)],
                vec![target(TypedDeclarationKind::Requirement, "req_typo")],
            ),
            "does not exist",
        ),
    ];

    for (input, message) in cases {
        let (_dir, store, scope) = initialized_store();
        let error = store.plan_typed_spec(&scope, input).unwrap_err();
        assert!(error.to_string().contains(message), "{error:#}");
        assert!(store.list_requirements(&scope).unwrap().is_empty());
    }
}

#[test]
fn adoption_conflicts_when_definition_or_source_relationship_differs() {
    let (_dir, store, scope) = initialized_store();
    create_unowned_requirement(&store, &scope, "req_existing", STATEMENT);
    let exact = document(
        OWNER,
        vec![requirement("canonical", Some("req_existing"), STATEMENT)],
        vec![target(TypedDeclarationKind::Requirement, "req_existing")],
    );
    let path = crate::shards::requirements_path(&store.layout, &scope);
    let before = std::fs::read(&path).unwrap();

    for mismatch in [
        {
            let mut value = exact.clone();
            value.requirements[0].statement = "A changed statement".to_string();
            value
        },
        {
            let mut value = exact.clone();
            value.requirements[0].description = Some("A changed description".to_string());
            value
        },
    ] {
        let plan = store.plan_typed_spec(&scope, mismatch.clone()).unwrap();
        assert_eq!(plan.conflicts, 1);
        assert!(plan.resources[0]
            .changes
            .iter()
            .any(|change| change.field == "statement" || change.field == "description"));
        assert!(store
            .apply_typed_spec(None, &scope, mismatch.clone())
            .is_err());
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }

    let mut relationship = exact;
    relationship.sources.push(TypedSourceInput {
        key: "policy".to_string(),
        id: Some("source_existing".to_string()),
        name: "Policy".to_string(),
        kind: "document".to_string(),
        url: None,
        reference: Some("docs/policy.md".to_string()),
    });
    relationship.requirements[0]
        .sources
        .push("policy".to_string());
    let plan = store.plan_typed_spec(&scope, relationship).unwrap();
    assert_eq!(plan.conflicts, 1);
    assert!(plan.resources[0]
        .changes
        .iter()
        .any(|change| change.field == "sources"));
}

#[test]
fn adoption_conflicts_when_source_metadata_differs() {
    let (_dir, store, scope) = initialized_store();
    create_unowned_source(&store, &scope, "External policy");
    let before = store.list_sources(&scope).unwrap();
    let mut input = document(
        OWNER,
        Vec::new(),
        vec![target(TypedDeclarationKind::Source, "source_policy")],
    );
    input.sources.push(TypedSourceInput {
        key: "policy".to_string(),
        id: Some("source_policy".to_string()),
        name: "Changed policy".to_string(),
        kind: "document".to_string(),
        url: None,
        reference: Some("docs/policy.md".to_string()),
    });

    let plan = store.plan_typed_spec(&scope, input.clone()).unwrap();
    assert_eq!(plan.conflicts, 1);
    assert!(plan.resources[0]
        .changes
        .iter()
        .any(|change| change.field == "name"));
    assert!(store.apply_typed_spec(None, &scope, input).is_err());
    assert_eq!(store.list_sources(&scope).unwrap(), before);
}

#[test]
fn mixed_valid_and_foreign_adoption_is_an_atomic_no_op() {
    let (_dir, store, scope) = initialized_store();
    create_unowned_requirement(
        &store,
        &scope,
        "req_valid",
        "The valid Requirement is stable",
    );
    store
        .apply_typed_spec(
            None,
            &scope,
            document(
                "spec://owner/a",
                vec![requirement(
                    "foreign",
                    Some("req_foreign"),
                    "The foreign Requirement is stable",
                )],
                Vec::new(),
            ),
        )
        .unwrap();
    let before = store.list_requirements(&scope).unwrap();
    let input = document(
        OWNER,
        vec![
            requirement(
                "valid",
                Some("req_valid"),
                "The valid Requirement is stable",
            ),
            requirement(
                "foreign",
                Some("req_foreign"),
                "The foreign Requirement is stable",
            ),
        ],
        vec![
            target(TypedDeclarationKind::Requirement, "req_valid"),
            target(TypedDeclarationKind::Requirement, "req_foreign"),
        ],
    );

    let plan = store.plan_typed_spec(&scope, input.clone()).unwrap();
    assert_eq!(plan.conflicts, 1);
    assert!(store.apply_typed_spec(None, &scope, input).is_err());
    assert_eq!(store.list_requirements(&scope).unwrap(), before);
}

#[test]
fn an_exact_unowned_rule_can_be_adopted_without_changing_its_relationship() {
    let (_dir, store, scope) = initialized_store();
    let base = document(
        OWNER,
        vec![requirement("canonical", Some("req_owned"), STATEMENT)],
        Vec::new(),
    );
    store.apply_typed_spec(None, &scope, base).unwrap();
    store
        .create_rule(
            None,
            CreateRuleInput {
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
            },
        )
        .unwrap();
    let mut input = document(
        OWNER,
        vec![requirement("canonical", Some("req_owned"), STATEMENT)],
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
        implementation: None,
    });

    let plan = store.plan_typed_spec(&scope, input.clone()).unwrap();
    assert_eq!((plan.created, plan.conflicts), (0, 0));
    store.apply_typed_spec(None, &scope, input).unwrap();
    let rule = store
        .list_rules(&scope)
        .unwrap()
        .into_iter()
        .find(|rule| rule.id.as_str() == "rule_existing")
        .unwrap();
    assert_eq!(rule.declared_by.as_deref(), Some(OWNER));
    assert_eq!(
        store
            .list_edges()
            .unwrap()
            .iter()
            .filter(|edge| edge.to_id.as_str() == "rule_existing")
            .count(),
        1
    );
}
