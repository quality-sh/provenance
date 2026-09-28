use crate::cache::ProjectionFamily;

use super::{
    all_families, graph_in_scope, implementation_binding_record, verification_binding_record,
    GraphReferenceError, ScopeId, SUPPORTED_SCHEMA_VERSION,
};

fn repository_with_bindings(
    verification: &provenance_core::VerificationBinding,
    implementation: &provenance_core::ImplementationBinding,
) -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    let root = camino::Utf8Path::from_path(directory.path()).unwrap();
    let layout = crate::layout::ProvenanceLayout::new(root);
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    std::fs::write(
        layout.manifest_path(),
        r#"{"schema_version":2,"scopes":[{"id":"default","path_prefix":"."}]}"#,
    )
    .unwrap();
    let scope = ScopeId::new("default").unwrap();
    crate::jsonl::write_jsonl_atomic(
        &crate::shards::verification_bindings_path(&layout, &scope),
        std::slice::from_ref(verification),
    )
    .unwrap();
    crate::jsonl::write_jsonl_atomic(
        &crate::shards::implementation_bindings_path(&layout, &scope),
        std::slice::from_ref(implementation),
    )
    .unwrap();
    directory
}

#[test]
fn graph_export_contains_each_table_field() {
    let scope = ScopeId::new("default").unwrap();
    let graph = graph_in_scope(&scope, &all_families());
    let value = serde_json::to_value(graph).unwrap();
    let object = value.as_object().unwrap();

    for family in ProjectionFamily::ALL {
        if let Some(field) = family.graph_field() {
            assert!(
                object.contains_key(field),
                "GraphExport does not contain the table field {field}"
            );
        }
    }
}

#[test]
fn schema_validation_checks_verification_bindings_before_implementation_bindings() {
    let scope = ScopeId::new("default").unwrap();
    let mut verification = verification_binding_record(&scope);
    let mut implementation = implementation_binding_record(&scope);
    let unsupported = provenance_core::SchemaVersion(SUPPORTED_SCHEMA_VERSION.0 + 1);
    verification.schema_version = unsupported;
    implementation.schema_version = unsupported;
    let directory = repository_with_bindings(&verification, &implementation);
    let root = camino::Utf8Path::from_path(directory.path()).unwrap();

    let GraphReferenceError::Incomplete { detail } =
        super::load_projection(root, "default").unwrap_err()
    else {
        panic!("unsupported binding schemas must make the graph incomplete");
    };
    assert!(detail.contains(verification.id.as_str()), "{detail}");
    assert!(!detail.contains(implementation.id.as_str()), "{detail}");
}

#[test]
fn scope_validation_checks_verification_bindings_before_implementation_bindings() {
    let scope = ScopeId::new("default").unwrap();
    let other = ScopeId::new("other").unwrap();
    let mut verification = verification_binding_record(&scope);
    let mut implementation = implementation_binding_record(&scope);
    verification.scope_id = other.clone();
    implementation.scope_id = other;
    let directory = repository_with_bindings(&verification, &implementation);
    let root = camino::Utf8Path::from_path(directory.path()).unwrap();

    let GraphReferenceError::Incomplete { detail } =
        super::load_projection(root, "default").unwrap_err()
    else {
        panic!("wrong-scope bindings must make the graph incomplete");
    };
    assert_eq!(
        detail,
        format!(
            "verification binding '{}' belongs to scope 'other', not 'default'",
            verification.id.as_str()
        )
    );
}
