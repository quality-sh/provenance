use super::seeded_requirement_store;
use provenance_core::ScopeId;
use provenance_core::SUPPORTED_SCHEMA_VERSION;

/// Appends one record the writers would refuse, the way a hand edit does.
fn append(path: &camino::Utf8Path, record: &serde_json::Value) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut contents = std::fs::read_to_string(path).unwrap_or_default();
    contents.push_str(&record.to_string());
    contents.push('\n');
    std::fs::write(path, contents).unwrap();
}

fn requirement(scope: &ScopeId, id: &str, refines: &str) -> serde_json::Value {
    serde_json::json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
        "scope_id": scope.as_str(),
        "id": id,
        "statement": format!("{id} statement"),
        "status": "active",
        "refines": refines,
    })
}

#[test]
fn a_scope_whose_relations_hold_passes() {
    let (_dir, store, scope) = seeded_requirement_store();
    store.validate_graph_scope(&scope).unwrap();
}

#[test]
fn a_rule_with_no_requirement_is_refused_by_the_validator() {
    let (_dir, store, scope) = seeded_requirement_store();
    append(
        &crate::shards::rules_path(&store.layout, &scope),
        &serde_json::json!({
            "schema_version": SUPPORTED_SCHEMA_VERSION.0,
            "scope_id": scope.as_str(),
            "id": "rule_bare",
            "statement": "A rule with no requirement",
            "status": "active",
            "severity": "high",
        }),
    );

    let error = store.validate_graph_scope(&scope).unwrap_err();

    assert_eq!(
        error.to_string(),
        "rule rule_bare is refused: a rule needs one requirement"
    );
}

#[test]
fn a_resolution_with_no_requirement_is_refused_by_the_validator() {
    let (_dir, store, scope) = seeded_requirement_store();
    append(
        &crate::shards::resolutions_path(&store.layout, &scope),
        &serde_json::json!({
            "schema_version": SUPPORTED_SCHEMA_VERSION.0,
            "scope_id": scope.as_str(),
            "id": "res_bare",
            "title": "Bare",
            "position": "Adopt",
            "rationale": "Nothing named",
            "status": "proposed",
        }),
    );

    let error = store.validate_graph_scope(&scope).unwrap_err();

    assert_eq!(
        error.to_string(),
        "resolution res_bare is refused: a resolution needs one requirement"
    );
}

#[test]
fn a_refines_cycle_in_state_is_refused_by_the_validator() {
    let (_dir, store, scope) = seeded_requirement_store();
    let path = crate::shards::requirements_path(&store.layout, &scope);
    append(&path, &requirement(&scope, "req_leave", "req_rates"));
    append(&path, &requirement(&scope, "req_rates", "req_leave"));

    let error = store.validate_graph_scope(&scope).unwrap_err();

    assert_eq!(
        error.to_string(),
        "refines forms a cycle: req_leave -> req_rates -> req_leave"
    );
}
