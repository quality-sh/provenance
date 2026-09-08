use super::write_error::{WriteError, WriteFailure};
use crate::{layout::ProvenanceLayout, state_store::StateStore};
use provenance_core::{Manifest, RepoPathPrefix, ScopeId};
use serde_json::json;

fn fixture() -> (tempfile::TempDir, StateStore, ScopeId) {
    let dir = tempfile::tempdir().unwrap();
    let layout = ProvenanceLayout::new(dir.path().to_str().unwrap());
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    let scope = ScopeId::new("default").unwrap();
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_vec(&Manifest::default_with_scope(
            scope.clone(),
            RepoPathPrefix::new("."),
        ))
        .unwrap(),
    )
    .unwrap();
    (dir, StateStore::new(layout), scope)
}
fn document() -> crate::state_store::TypedSpecInput {
    serde_json::from_value(json!({"schema_version":provenance_core::SUPPORTED_SCHEMA_VERSION.0,"spec":"example","declared_by":"test",
        "sources":[{"key":"policy","name":"Policy","kind":"document"}],
        "requirements":[{"key":"ready","statement":"The system is ready.","sources":["policy"]}],
        "rules":[{"key":"ready","requirement":"ready","statement":"The system is ready."}]})).unwrap()
}
#[test]
fn missing_references_are_typed_without_changing_the_native_message() {
    let (_dir, store, scope) = fixture();
    let mut input = document();
    input.requirements[0].sources = vec!["absent".into()];
    let error = WriteError(store.apply_typed_spec(&scope, input).unwrap_err());
    assert!(error.to_string().contains("absent"));
    assert!(matches!(error.safe(), WriteFailure::MissingReference));
}
#[test]
fn invalid_source_kind_is_a_typed_declaration_refusal() {
    let (_dir, store, scope) = fixture();
    let mut input = document();
    input.sources[0].kind = "nonsense".into();
    let error = WriteError(store.apply_typed_spec(&scope, input).unwrap_err());
    assert_eq!(error.to_string(), "source kind `nonsense` is not supported");
    assert!(matches!(error.safe(), WriteFailure::InvalidDeclaration));
}
#[test]
fn apply_reports_uncertainty_after_real_source_publication() {
    let (_dir, store, scope) = fixture();
    crate::test_probes::arm("typed_spec_sources_published", || {
        anyhow::bail!("injected after source publication")
    });
    let result = store.apply_typed_spec(&scope, document());
    crate::test_probes::disarm("typed_spec_sources_published");
    let error = WriteError(result.unwrap_err());
    assert!(matches!(error.safe(), WriteFailure::UncertainWrite));
    assert_eq!(store.list_sources(&scope).unwrap().len(), 1);
    assert!(store.list_requirements(&scope).unwrap().is_empty());
}
#[test]
fn already_complete_is_typed_and_retains_native_text() {
    let (_dir, store, scope) = fixture();
    let result = store.apply_typed_spec(&scope, document()).unwrap();
    let rule = result
        .resources
        .iter()
        .find(|value| value.kind == crate::state_store::TypedResourceKind::Rule)
        .unwrap();
    let input = serde_json::from_value(json!({"rule":rule.id,"key":"test","method":"examples","declared_by":"test","file":"test.rs"})).unwrap();
    let run = store.begin_verification(scope.clone(), input).unwrap();
    let complete = || serde_json::from_value(json!({"run":run.id,"status":"passed"})).unwrap();
    store.complete_verification(&scope, complete()).unwrap();
    let error = WriteError(store.complete_verification(&scope, complete()).unwrap_err());
    assert!(error.to_string().contains("already complete"));
    assert!(matches!(error.safe(), WriteFailure::AlreadyComplete));
}
#[test]
fn native_schema_diagnostic_is_not_duplicated_in_the_error_chain() {
    let (_dir, store, scope) = fixture();
    let mut input = document();
    input.schema_version = 999;
    let error = store.apply_typed_spec(&scope, input).unwrap_err();
    assert_eq!(
        format!("{error:#}"),
        format!(
            "typed spec schema_version must be {}",
            provenance_core::SUPPORTED_SCHEMA_VERSION.0
        )
    );
}
#[test]
fn begin_keeps_the_publication_lock_and_reports_saved_binding_on_failure() {
    let (_dir, store, scope) = fixture();
    let result = store.apply_typed_spec(&scope, document()).unwrap();
    let rule = result
        .resources
        .iter()
        .find(|value| value.kind == crate::state_store::TypedResourceKind::Rule)
        .unwrap();
    let layout = store.layout.clone();
    crate::test_probes::arm("verification_binding_published", move || {
        assert!(crate::test_probes::publication_lock_is_held(&layout));
        anyhow::bail!("injected after binding")
    });
    let result = store.begin_verification(scope.clone(), serde_json::from_value(json!({"rule":rule.id,"key":"test","method":"examples","declared_by":"test","file":"test.rs"})).unwrap());
    crate::test_probes::disarm("verification_binding_published");
    assert!(matches!(
        WriteError(result.unwrap_err()).safe(),
        WriteFailure::UncertainWrite
    ));
    assert_eq!(store.list_verification_bindings(&scope).unwrap().len(), 1);
    assert!(store.list_verification_runs(&scope).unwrap().is_empty());
}

#[test]
fn implementation_target_validation_is_typed_before_publication() {
    let (dir, store, scope) = fixture();
    std::fs::write(dir.path().join("test.rs"), "fn check() {}\n").unwrap();
    let mut input = document();
    input.rules[0].implementation = Some(provenance_core::protocol::TypedImplementationInput {
        file: "test.rs".into(),
        symbol: String::new(),
    });
    let error = WriteError(store.apply_typed_spec(&scope, input).unwrap_err());
    assert_eq!(error.to_string(), "implementation symbol must not be empty");
    assert!(matches!(error.safe(), WriteFailure::InvalidDeclaration));
    assert!(store.list_rules(&scope).unwrap().is_empty());
}

#[test]
fn foreign_implementation_owner_is_a_typed_conflict_with_its_identity() {
    let (dir, store, scope) = fixture();
    std::fs::write(dir.path().join("test.rs"), "fn check() {}\n").unwrap();
    let result = store.apply_typed_spec(&scope, document()).unwrap();
    let rule = result
        .resources
        .iter()
        .find(|value| value.kind == crate::state_store::TypedResourceKind::Rule)
        .unwrap();
    store
        .materialize_implementation_binding(
            crate::state_store::MaterializeImplementationBindingInput {
                scope_id: scope.clone(),
                rule_id: rule.id.clone(),
                declared_by: "foreign".into(),
                file: "test.rs".into(),
                symbol: "check".into(),
            },
        )
        .unwrap();
    let mut input = document();
    input.rules[0].implementation = Some(provenance_core::protocol::TypedImplementationInput {
        file: "test.rs".into(),
        symbol: "check".into(),
    });
    let error = WriteError(store.apply_typed_spec(&scope, input).unwrap_err());
    assert_eq!(
        error.to_string(),
        format!(
            "rule `{}` implementation is owned by `foreign`",
            rule.id.as_str()
        )
    );
    let WriteFailure::OwnershipConflict { conflicts } = error.safe() else {
        panic!("expected ownership conflict")
    };
    assert_eq!(conflicts[0].id, rule.id);
    assert_eq!(conflicts[0].changes[0].before, "foreign");
}
