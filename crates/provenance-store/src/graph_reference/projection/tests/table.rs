use crate::cache::ProjectionFamily;

use super::{
    all_families, graph_in_scope, GraphReferenceError, RecordFamily, ScopeId, StableId,
    SUPPORTED_SCHEMA_VERSION,
};

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
    let mut graph = graph_in_scope(&scope, &all_families());
    graph.verification_bindings[0].schema_version = SUPPORTED_SCHEMA_VERSION.0 + 1;
    graph.implementation_bindings[0].schema_version = SUPPORTED_SCHEMA_VERSION.0 + 1;

    let GraphReferenceError::Incomplete { detail } = graph.validate_schema_versions().unwrap_err()
    else {
        panic!("unsupported binding schemas must make the graph incomplete");
    };
    assert!(detail.contains("verification binding"), "{detail}");
}

#[test]
fn scope_validation_checks_verification_bindings_before_implementation_bindings() {
    let scope = ScopeId::new("default").unwrap();
    let other = ScopeId::new("other").unwrap();
    let mut graph = graph_in_scope(&scope, &all_families());
    graph.verification_bindings[0].scope_id = other.clone();
    graph.implementation_bindings[0].scope_id = other;

    let GraphReferenceError::Incomplete { detail } =
        super::validate_scope_ownership(&graph, &scope).unwrap_err()
    else {
        panic!("wrong-scope bindings must make the graph incomplete");
    };
    assert_eq!(
        detail,
        format!(
            "verification binding '{}' belongs to scope 'other', not 'default'",
            StableId::new(super::record_id(RecordFamily::VerificationBinding)).unwrap()
        )
    );
}
