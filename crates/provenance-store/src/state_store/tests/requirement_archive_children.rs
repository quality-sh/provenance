use super::requirement_archive::{
    append_rule, archive, assert_invalid_update, child, requirement, COMMIT,
};
use super::seeded_requirement_store;
use crate::state_store::StateStore;
use provenance_core::{Resolution, Rule, ScopeId};
use provenance_macros::verifies;
use serde_json::{json, Value};

fn create_rule(store: &StateStore, id: &str, parents: &[&str]) {
    store
        .create_rule(
            serde_json::from_value(json!({
                "scope_id":"default", "id":id, "statement":"The system is safe.",
                "status":"active", "severity":"medium", "requirement_ids":parents,
                "resolution_ids":[]
            }))
            .unwrap(),
        )
        .unwrap();
}

fn create_resolution(store: &StateStore, id: &str, parents: &[&str]) {
    store
        .create_resolution(
            serde_json::from_value(json!({
                "scope_id":"default", "id":id, "title":"Keep the store.",
                "requirement_ids":parents, "supersedes":[], "position":"The store keeps it.",
                "rationale":"It is simple.", "status":"proposed", "inputs":[]
            }))
            .unwrap(),
        )
        .unwrap();
}

fn set_resolution_status(store: &StateStore, id: &str, status: &str) {
    store
        .update_resolution(
            serde_json::from_value(json!({"scope_id":"default", "id":id, "status":status}))
                .unwrap(),
        )
        .unwrap();
}

fn rule(store: &StateStore, scope: &ScopeId, id: &str) -> Rule {
    store
        .list_rules(scope)
        .unwrap()
        .into_iter()
        .find(|record| record.id.as_str() == id)
        .unwrap()
}

fn resolution(store: &StateStore, scope: &ScopeId, id: &str) -> Resolution {
    store
        .list_resolutions(scope)
        .unwrap()
        .into_iter()
        .find(|record| record.id.as_str() == id)
        .unwrap()
}

/// Removes the fields a pruned link is allowed to change.
fn without_link_fields(record: &impl serde::Serialize) -> Value {
    let mut value = serde_json::to_value(record).unwrap();
    for key in ["updated", "requirement_ids"] {
        value.as_object_mut().unwrap().remove(key);
    }
    value
}

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
fn rule_under_two_cascaded_parents_is_archived_with_its_links() {
    let (_directory, store, scope) = sibling_store();
    create_rule(&store, "rule_shared", &["req_left", "req_right"]);
    archive(&store, "req_overtime");
    let after = serde_json::to_value(rule(&store, &scope, "rule_shared")).unwrap();
    assert_eq!(after["status"], "archived");
    assert_eq!(after["requirement_ids"], json!(["req_left", "req_right"]));
    assert_eq!(after["archived_in_commit"]["commit"], COMMIT);
}

#[test]
#[verifies("rule_archive_preserves_surviving_children", examples)]
#[verifies("rule_archive_prunes_only_parent_edge", examples)]
fn rule_with_a_surviving_parent_loses_only_the_archived_link() {
    let (_directory, store, scope) = sibling_store();
    create_rule(&store, "rule_shared", &["req_left", "req_right"]);
    let before = rule(&store, &scope, "rule_shared");
    archive(&store, "req_left");
    let after = rule(&store, &scope, "rule_shared");
    assert_eq!(
        after.requirement_ids,
        [requirement(&store, &scope, "req_right").id]
    );
    assert_eq!(without_link_fields(&after), without_link_fields(&before));
}

#[test]
#[verifies("rule_archive_cascades_without_surviving_parents", examples)]
#[verifies("rule_live_dependents_have_live_parents", examples)]
fn already_archived_parent_does_not_keep_a_child_live() {
    let (_directory, store, scope) = sibling_store();
    archive(&store, "req_left");
    append_rule(&store, &scope, "rule_merged", &["req_left", "req_right"]);
    archive(&store, "req_right");
    let after = serde_json::to_value(rule(&store, &scope, "rule_merged")).unwrap();
    assert_eq!(after["status"], "archived");
    assert_eq!(after["requirement_ids"], json!(["req_left", "req_right"]));
    assert_eq!(after["archived_in_commit"]["commit"], COMMIT);
}

#[test]
#[verifies("rule_archive_cascades_without_surviving_parents", examples)]
#[verifies("rule_archive_keeps_archived_parent_links", examples)]
#[verifies("rule_archive_records_each_commit", examples)]
fn resolution_under_an_archived_requirement_is_archived() {
    let (_directory, store, scope) = seeded_requirement_store();
    create_resolution(&store, "res_only", &["req_overtime"]);
    archive(&store, "req_overtime");
    let after = serde_json::to_value(resolution(&store, &scope, "res_only")).unwrap();
    assert_eq!(after["status"], "archived");
    assert_eq!(after["requirement_ids"], json!(["req_overtime"]));
    assert_eq!(after["archived_in_commit"]["commit"], COMMIT);
}

#[test]
#[verifies("rule_archive_preserves_surviving_children", examples)]
#[verifies("rule_archive_prunes_only_parent_edge", examples)]
fn resolution_with_a_surviving_parent_loses_only_the_archived_link() {
    let (_directory, store, scope) = sibling_store();
    create_resolution(&store, "res_shared", &["req_left", "req_right"]);
    let before = resolution(&store, &scope, "res_shared");
    archive(&store, "req_left");
    let after = resolution(&store, &scope, "res_shared");
    assert_eq!(
        after.requirement_ids,
        [requirement(&store, &scope, "req_right").id]
    );
    assert_eq!(without_link_fields(&after), without_link_fields(&before));
}

#[test]
#[verifies("rule_archive_is_terminal_for_each_record", examples)]
fn archived_resolution_refuses_a_live_status() {
    let (_directory, store, scope) = seeded_requirement_store();
    create_resolution(&store, "res_only", &["req_overtime"]);
    archive(&store, "req_overtime");
    let archived = resolution(&store, &scope, "res_only");
    let error = store
        .update_resolution(
            serde_json::from_value(
                json!({"scope_id":"default", "id":"res_only", "status":"proposed"}),
            )
            .unwrap(),
        )
        .unwrap_err();
    assert_invalid_update(error);
    assert_eq!(resolution(&store, &scope, "res_only"), archived);
}

#[test]
#[verifies("rule_archive_cascades_without_surviving_parents", examples)]
#[verifies("rule_archive_preserves_surviving_children", examples)]
fn ended_resolutions_keep_their_status_and_links() {
    let (_directory, store, scope) = sibling_store();
    create_resolution(&store, "res_abandoned", &["req_left"]);
    set_resolution_status(&store, "res_abandoned", "abandoned");
    create_resolution(&store, "res_rejected", &["req_left", "req_right"]);
    set_resolution_status(&store, "res_rejected", "rejected");
    let abandoned = resolution(&store, &scope, "res_abandoned");
    let rejected = resolution(&store, &scope, "res_rejected");
    archive(&store, "req_left");
    assert_eq!(resolution(&store, &scope, "res_abandoned"), abandoned);
    assert_eq!(resolution(&store, &scope, "res_rejected"), rejected);
}
