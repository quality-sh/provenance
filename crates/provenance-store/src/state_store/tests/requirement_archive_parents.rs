use super::requirement_archive::{archive, child, requirement, COMMIT};
use super::seeded_requirement_store;
use crate::state_store::StateStore;
use provenance_core::{ScopeId, SUPPORTED_SCHEMA_VERSION};
use provenance_macros::verifies;
use serde_json::{json, Value};

fn create(store: &StateStore, id: &str, parents: &[&str]) {
    store
        .create_requirement(
            serde_json::from_value(json!({
                "scope_id":"default", "id":id, "statement":"The system is safe.",
                "status":"active", "refines":parents, "depends_on":[], "supersedes":[]
            }))
            .unwrap(),
        )
        .unwrap();
}

/// Appends one Requirement as a merge of two branches can leave it.
fn append(store: &StateStore, scope: &ScopeId, id: &str, parents: &[&str]) {
    let path = crate::shards::requirements_path(&store.layout, scope);
    let mut contents = std::fs::read_to_string(&path).unwrap_or_default();
    contents.push_str(
        &json!({
            "schema_version": SUPPORTED_SCHEMA_VERSION.0, "scope_id": scope.as_str(),
            "id": id, "statement": "The system is safe.", "status": "active",
            "refines": parents
        })
        .to_string(),
    );
    contents.push('\n');
    std::fs::write(&path, contents).unwrap();
}

fn value(store: &StateStore, scope: &ScopeId, id: &str) -> Value {
    serde_json::to_value(requirement(store, scope, id)).unwrap()
}

/// `req_left` and `req_right` both refine `req_overtime`.
fn sibling_store() -> (tempfile::TempDir, StateStore, ScopeId) {
    let (directory, store, scope) = seeded_requirement_store();
    child(&store, "req_left", "req_overtime");
    child(&store, "req_right", "req_overtime");
    (directory, store, scope)
}

#[test]
#[verifies("rule_archive_cascades_without_surviving_parents", examples)]
#[verifies("rule_archive_keeps_archived_parent_links", examples)]
#[verifies("rule_archive_records_each_commit", examples)]
fn requirement_under_two_cascaded_parents_is_archived_with_its_links() {
    let (_directory, store, scope) = sibling_store();
    create(&store, "req_both", &["req_left", "req_right"]);
    archive(&store, "req_overtime");
    let after = value(&store, &scope, "req_both");
    assert_eq!(after["status"], "archived");
    assert_eq!(after["refines"], json!(["req_left", "req_right"]));
    assert_eq!(after["archived_in_commit"]["commit"], COMMIT);
}

#[test]
#[verifies("rule_archive_preserves_surviving_children", examples)]
#[verifies("rule_archive_prunes_only_parent_edge", examples)]
#[verifies("rule_archive_does_not_create_roots", examples)]
fn requirement_with_a_surviving_parent_loses_only_the_archived_link() {
    let (_directory, store, scope) = seeded_requirement_store();
    create(&store, "req_kept", &[]);
    create(&store, "req_both", &["req_kept", "req_overtime"]);
    let mut before = value(&store, &scope, "req_both");
    archive(&store, "req_overtime");
    let mut after = value(&store, &scope, "req_both");
    assert_eq!(after["status"], "active");
    assert_eq!(after["refines"], json!(["req_kept"]));
    for record in [&mut before, &mut after] {
        let fields = record.as_object_mut().unwrap();
        fields.remove("refines");
        fields.remove("updated");
    }
    assert_eq!(after, before);
}

#[test]
#[verifies("rule_archive_cascades_without_surviving_parents", examples)]
#[verifies("rule_live_dependents_have_live_parents", examples)]
fn already_archived_parent_does_not_keep_a_requirement_live() {
    let (_directory, store, scope) = sibling_store();
    archive(&store, "req_left");
    append(&store, &scope, "req_merged", &["req_left", "req_right"]);
    archive(&store, "req_right");
    let after = value(&store, &scope, "req_merged");
    assert_eq!(after["status"], "archived");
    assert_eq!(after["refines"], json!(["req_left", "req_right"]));
    assert_eq!(after["archived_in_commit"]["commit"], COMMIT);
}
