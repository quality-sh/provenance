use super::seeded_requirement_store;
use crate::state_store::{
    BeginVerificationInput, CreateRuleInput, MaterializeVerificationBindingInput,
};
use provenance_core::{RuleSeverity, RuleStatus, StableId, VerificationMethod};
use provenance_macros::verifies;

fn seeded_rule_store() -> (
    tempfile::TempDir,
    super::StateStore,
    provenance_core::ScopeId,
) {
    let (directory, store, scope) = seeded_requirement_store();
    store
        .create_rule(CreateRuleInput {
            archived_in_commit: None,
            scope_id: scope.clone(),
            id: StableId::new("rule_expiry").unwrap(),
            name: None,
            description: None,
            requirement_ids: vec![StableId::new("req_overtime").unwrap()],
            resolution_ids: Vec::new(),
            statement: "Share links expire".into(),
            status: RuleStatus::Active,
            severity: RuleSeverity::Medium,
            source_document: None,
            source_section: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
    (directory, store, scope)
}

fn input(scope: &provenance_core::ScopeId) -> MaterializeVerificationBindingInput {
    MaterializeVerificationBindingInput {
        scope_id: scope.clone(),
        rule_id: StableId::new("rule_expiry").unwrap(),
        key: "share-link-expiry".into(),
        method: VerificationMethod::Examples,
        declared_by: "test://typescript".into(),
        file: "tests/share-links.test.ts".into(),
        symbol: Some("share links expire".into()),
    }
}

#[test]
fn repeated_materialization_updates_one_binding_with_stable_identity() {
    let (_directory, store, scope) = seeded_rule_store();
    let first = store
        .materialize_verification_binding(input(&scope))
        .unwrap();
    let mut changed = input(&scope);
    changed.method = VerificationMethod::Property;
    changed.file = "tests/property.test.ts".into();
    changed.symbol = Some("expiry property".into());

    let second = store.materialize_verification_binding(changed).unwrap();
    let stored = store.list_verification_bindings(&scope).unwrap();

    assert_eq!(first.id, second.id);
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].method, VerificationMethod::Property);
    assert_eq!(
        stored[0].file,
        camino::Utf8PathBuf::from("tests/property.test.ts")
    );
}

#[test]
fn identity_changes_with_owner_rule_or_explicit_key() {
    let (_directory, store, scope) = seeded_rule_store();
    let first = store
        .materialize_verification_binding(input(&scope))
        .unwrap();
    let mut another_key = input(&scope);
    another_key.key = "another-check".into();
    let second = store.materialize_verification_binding(another_key).unwrap();
    let mut another_owner = input(&scope);
    another_owner.declared_by = "test://another".into();
    let third = store
        .materialize_verification_binding(another_owner)
        .unwrap();

    assert_ne!(first.id, second.id);
    assert_ne!(first.id, third.id);
    assert_eq!(store.list_verification_bindings(&scope).unwrap().len(), 3);
}

#[test]
#[verifies("rule_porcelain_id_unique_in_repository", examples)]
fn canonical_identity_cannot_be_reused_in_another_scope() {
    let (_directory, store, _scope) = seeded_rule_store();
    let other_scope = provenance_core::ScopeId::new("other").unwrap();
    let mut manifest = store.manifest().unwrap();
    manifest.scopes.push(provenance_core::Scope {
        id: other_scope.clone(),
        path_prefix: provenance_core::RepoPathPrefix::new("other"),
    });
    std::fs::write(
        store.layout.manifest_path(),
        serde_json::to_string(&manifest).unwrap(),
    )
    .unwrap();
    store
        .create_requirement(crate::state_store::CreateRequirementInput {
            scope_id: other_scope.clone(),
            id: StableId::new("req_other").unwrap(),
            statement: "Other".into(),
            description: None,
            status: provenance_core::RequirementStatus::Active,
            domain_id: None,
            refines: None,
            depends_on: Vec::new(),
            supersedes: Vec::new(),
            spawned_by: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
    let error = store
        .create_rule(CreateRuleInput {
            archived_in_commit: None,
            scope_id: other_scope,
            id: StableId::new("rule_expiry").unwrap(),
            name: None,
            description: None,
            requirement_ids: vec![StableId::new("req_other").unwrap()],
            resolution_ids: Vec::new(),
            statement: "Share links expire".into(),
            status: RuleStatus::Active,
            severity: RuleSeverity::Medium,
            source_document: None,
            source_section: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap_err();

    assert!(error.to_string().contains("record ID already exists"));
}

#[test]
fn materialization_requires_a_known_rule_and_filled_identity_fields() {
    let (_directory, store, scope) = seeded_rule_store();
    let mut unknown = input(&scope);
    unknown.rule_id = StableId::new("rule_missing").unwrap();
    assert!(store
        .materialize_verification_binding(unknown)
        .unwrap_err()
        .to_string()
        .contains("does not exist"));

    let mut blank_key = input(&scope);
    blank_key.key = "  ".into();
    assert!(store
        .materialize_verification_binding(blank_key)
        .unwrap_err()
        .to_string()
        .contains("key must not be empty"));

    let mut blank_owner = input(&scope);
    blank_owner.declared_by = String::new();
    assert!(store
        .materialize_verification_binding(blank_owner)
        .unwrap_err()
        .to_string()
        .contains("declared_by must not be empty"));

    let mut outside = input(&scope);
    outside.file = "../outside.test.ts".into();
    assert!(store
        .materialize_verification_binding(outside)
        .unwrap_err()
        .to_string()
        .contains("repository-relative"));

    let mut platform_specific = input(&scope);
    platform_specific.file = r"tests\share-links.test.ts".into();
    assert!(store
        .materialize_verification_binding(platform_specific)
        .unwrap_err()
        .to_string()
        .contains("repository-relative"));
}

#[test]
fn beginning_a_run_materializes_and_cites_the_canonical_binding() {
    let (_directory, store, scope) = seeded_rule_store();

    let run = store
        .begin_verification(
            scope.clone(),
            BeginVerificationInput {
                rule: Some("rule_expiry".into()),
                declaration: None,
                key: "share-link-expiry".into(),
                method: "examples".into(),
                declared_by: "test://typescript".into(),
                file: Some("tests/share-links.test.ts".into()),
                symbol: Some("share links expire".into()),
                commit: Some("0123456789abcdef0123456789abcdef01234567".into()),
            },
            VerificationMethod::Examples,
        )
        .unwrap();
    let bindings = store.list_verification_bindings(&scope).unwrap();

    assert_eq!(bindings.len(), 1);
    assert_eq!(run.binding_id.as_ref(), Some(&bindings[0].id));
    assert_eq!(
        run.commit.as_deref(),
        Some("0123456789abcdef0123456789abcdef01234567")
    );
    assert_eq!(run.file, Some(bindings[0].file.clone()));
    assert_eq!(run.symbol, bindings[0].symbol);
}

#[tokio::test]
async fn the_verification_page_lists_only_the_bindings_of_the_named_rule() {
    let (directory, store, scope) = seeded_rule_store();
    let binding = store
        .materialize_verification_binding(input(&scope))
        .unwrap();
    let root = camino::Utf8PathBuf::from_path_buf(directory.path().to_path_buf()).unwrap();

    let named = verification_page(&root, "rule_expiry").await;
    let items = named["result"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["id"], binding.id.as_str());
    assert_eq!(named["result"]["has_more"], false);

    let other = verification_page(&root, "rule_other").await;
    assert!(other["result"]["items"].as_array().unwrap().is_empty());
}

async fn verification_page(root: &camino::Utf8Path, rule: &str) -> serde_json::Value {
    super::read_budget::operation_read(
        root,
        "page-verification-bindings-v2",
        serde_json::json!({"rule": rule, "limit": 10, "cursor": null}),
    )
    .await
    .unwrap()
}
