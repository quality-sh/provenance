use super::write_error::{WriteError, WriteFailure};
use crate::{layout::ProvenanceLayout, state_store::StateStore};
use provenance_core::{Manifest, RepoPathPrefix, ScopeId, VerificationMethod};
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
fn publication_document(
    requirement_statement: &str,
    include_retired: bool,
    implementation: bool,
) -> crate::state_store::TypedSpecInput {
    let mut value = json!({
        "schema_version":provenance_core::SUPPORTED_SCHEMA_VERSION.0,
        "spec":"publication", "declared_by":"test",
        "sources":[{"key":"policy","name":"Policy","kind":"document"}],
        "requirements":[{
            "key":"ready", "statement":requirement_statement, "sources":["policy"]
        }],
        "rules":[{
            "key":"ready", "requirement":"ready", "statement":"The system is ready."
        }]
    });
    if include_retired {
        value["requirements"].as_array_mut().unwrap().push(json!({
            "key":"retired", "statement":"The old system is ready."
        }));
        value["rules"].as_array_mut().unwrap().push(json!({
            "key":"retired", "requirement":"retired",
            "statement":"The old system is ready."
        }));
    }
    if implementation {
        value["sources"][0]["name"] = json!("Revised policy");
        value["rules"][0]["implementation"] = json!({"file":"src/ready.rs","symbol":"ready"});
    }
    serde_json::from_value(value).unwrap()
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
fn typed_apply_failure_after_first_write_keeps_the_canonical_graph_unchanged() {
    let (_dir, store, scope) = fixture();
    let before = canonical_state(&store);
    crate::test_probes::arm("typed_spec_sources_published", || {
        anyhow::bail!("injected after source publication")
    });
    let result = store.apply_typed_spec(&scope, document());
    crate::test_probes::disarm("typed_spec_sources_published");
    let error = WriteError(result.unwrap_err());
    assert!(matches!(error.safe(), WriteFailure::WriteFailed));
    assert_eq!(canonical_state(&store), before);
    assert!(store.list_sources(&scope).unwrap().is_empty());
    assert!(store.list_requirements(&scope).unwrap().is_empty());
}

#[test]
fn requirement_update_failure_after_first_write_keeps_statement_and_reviews_together() {
    let (_dir, store, scope) = fixture();
    store
        .create_requirement(
            serde_json::from_value(json!({
                "scope_id":"default", "id":"req_a",
                "statement":"The system stores records.", "status":"active",
                "depends_on":[], "supersedes":[]
            }))
            .unwrap(),
        )
        .unwrap();
    store
        .create_rule(
            serde_json::from_value(json!({
                "scope_id":"default", "id":"rule_a",
                "statement":"The system stores the record.", "status":"active",
                "severity":"high", "requirement_ids":["req_a"], "resolution_ids":[]
            }))
            .unwrap(),
        )
        .unwrap();
    let before = canonical_state(&store);
    crate::test_probes::crash_at("requirement_update_record_written");
    let result = store.update_requirement(
        serde_json::from_value(json!({
            "scope_id":"default", "id":"req_a",
            "statement":"The system reads records."
        }))
        .unwrap(),
    );
    crate::test_probes::disarm("requirement_update_record_written");

    assert!(result.is_err());
    assert_eq!(canonical_state(&store), before);
    assert_eq!(
        store.list_requirements(&scope).unwrap()[0].statement,
        "The system stores records."
    );
    assert!(store.open_requirement_reviews(&scope).unwrap().is_empty());
}

#[test]
fn typed_apply_success_publishes_records_cascade_and_review_together() {
    let (_dir, store, scope) = fixture();
    store
        .apply_typed_spec(
            &scope,
            publication_document("The system is ready.", true, false),
        )
        .unwrap();
    let retired = store
        .list_requirements(&scope)
        .unwrap()
        .into_iter()
        .find(|record| record.statement == "The old system is ready.")
        .unwrap();
    store
        .create_boundary(
            serde_json::from_value(json!({
                "scope_id":"default", "id":"boundary_retired",
                "requirement_id":retired.id, "statement":"Retired boundary"
            }))
            .unwrap(),
        )
        .unwrap();

    store
        .apply_typed_spec(
            &scope,
            publication_document("The revised system is ready.", false, true),
        )
        .unwrap();

    assert_eq!(
        store.list_sources(&scope).unwrap()[0].name,
        "Revised policy"
    );
    assert_eq!(
        store.list_requirements(&scope).unwrap()[0].statement,
        "The revised system is ready."
    );
    assert_eq!(store.list_rules(&scope).unwrap().len(), 1);
    assert_eq!(store.list_implementation_bindings(&scope).unwrap().len(), 1);
    assert!(store.list_boundaries(&scope).unwrap().is_empty());
    let reviews = store.open_requirement_reviews(&scope).unwrap();
    assert_eq!(reviews.len(), 1);
    assert_eq!(reviews[0].before, "The system is ready.");
    assert_eq!(reviews[0].after, "The revised system is ready.");
}

fn canonical_state(store: &StateStore) -> Vec<(String, Vec<u8>)> {
    let root = store.layout.state_dir();
    let mut state = Vec::new();
    collect_state(&root, &root, &mut state);
    state.sort_by(|left, right| left.0.cmp(&right.0));
    state
}

fn collect_state(
    root: &camino::Utf8Path,
    current: &camino::Utf8Path,
    state: &mut Vec<(String, Vec<u8>)>,
) {
    for entry in std::fs::read_dir(current).unwrap() {
        let path = camino::Utf8PathBuf::from_path_buf(entry.unwrap().path()).unwrap();
        if path.is_dir() {
            collect_state(root, &path, state);
        } else {
            state.push((
                path.strip_prefix(root).unwrap().to_string(),
                std::fs::read(path).unwrap(),
            ));
        }
    }
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
    let run = store
        .begin_verification(scope.clone(), input, VerificationMethod::Examples)
        .unwrap();
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
    let result = store.begin_verification(
        scope.clone(),
        serde_json::from_value(json!({"rule":rule.id,"key":"test","method":"examples","declared_by":"test","file":"test.rs"})).unwrap(),
        VerificationMethod::Examples,
    );
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
