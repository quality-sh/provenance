use crate::cache::ProjectionFamily;

use super::{all_families, graph_in_scope, ScopeId};

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
