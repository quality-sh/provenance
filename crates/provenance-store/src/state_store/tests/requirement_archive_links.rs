use super::requirement_archive::{archive, assert_invalid_update, child};
use super::seeded_requirement_store;
use crate::state_store::StateStore;
use provenance_core::ScopeId;
use provenance_macros::verifies;
use serde_json::json;

/// Archives `req_old`, which refines `req_overtime`; `req_overtime` stays live.
fn archived_parent_store() -> (tempfile::TempDir, StateStore, ScopeId) {
    let (directory, store, scope) = seeded_requirement_store();
    child(&store, "req_old", "req_overtime");
    archive(&store, "req_old");
    (directory, store, scope)
}

fn rule_input(id: &str, parent: &str) -> serde_json::Value {
    json!({
        "scope_id":"default", "id":id, "statement":"The system is safe.",
        "status":"draft", "severity":"medium", "requirement_ids":[parent],
        "resolution_ids":[]
    })
}

fn resolution_input(id: &str, parent: &str) -> serde_json::Value {
    json!({
        "scope_id":"default", "id":id, "title":"Keep the store.",
        "requirement_ids":[parent], "supersedes":[], "position":"The store keeps it.",
        "rationale":"It is simple.", "status":"proposed", "inputs":[]
    })
}

#[test]
#[verifies("rule_live_dependents_have_live_parents", examples)]
fn new_requirement_cannot_refine_an_archived_requirement() {
    let (_directory, store, scope) = archived_parent_store();
    let before = store.list_requirements(&scope).unwrap();
    let error = store
        .create_requirement(
            serde_json::from_value(json!({
                "scope_id":"default", "id":"req_late", "statement":"The system is safe.",
                "status":"active", "refines":["req_old"], "depends_on":[], "supersedes":[]
            }))
            .unwrap(),
        )
        .unwrap_err();
    assert_invalid_update(error);
    assert_eq!(store.list_requirements(&scope).unwrap(), before);
}

#[test]
#[verifies("rule_live_dependents_have_live_parents", examples)]
fn new_rule_cannot_link_an_archived_requirement() {
    let (_directory, store, scope) = archived_parent_store();
    let before = store.list_rules(&scope).unwrap();
    let error = store
        .create_rule(serde_json::from_value(rule_input("rule_late", "req_old")).unwrap())
        .unwrap_err();
    assert_invalid_update(error);
    assert_eq!(store.list_rules(&scope).unwrap(), before);
}

#[test]
#[verifies("rule_live_dependents_have_live_parents", examples)]
fn new_resolution_cannot_link_an_archived_requirement() {
    let (_directory, store, scope) = archived_parent_store();
    let before = store.list_resolutions(&scope).unwrap();
    let error = store
        .create_resolution(serde_json::from_value(resolution_input("res_late", "req_old")).unwrap())
        .unwrap_err();
    assert_invalid_update(error);
    assert_eq!(store.list_resolutions(&scope).unwrap(), before);
}

#[test]
#[verifies("rule_live_dependents_have_live_parents", examples)]
fn new_topic_cannot_link_an_archived_requirement() {
    let (_directory, store, scope) = archived_parent_store();
    let before = store.list_topics(&scope).unwrap();
    let error = store
        .create_topic(
            serde_json::from_value(json!({
                "scope_id":"default", "id":"topic_late", "requirement_id":"req_old",
                "title":"Stored history", "status":"open", "links":[]
            }))
            .unwrap(),
        )
        .unwrap_err();
    assert_invalid_update(error);
    assert_eq!(store.list_topics(&scope).unwrap(), before);
}

#[test]
#[verifies("rule_live_dependents_have_live_parents", examples)]
fn rule_update_cannot_add_an_archived_requirement() {
    let (_directory, store, scope) = archived_parent_store();
    store
        .create_rule(serde_json::from_value(rule_input("rule_live", "req_overtime")).unwrap())
        .unwrap();
    let before = store.list_rules(&scope).unwrap();
    let error = store
        .update_rule(
            serde_json::from_value(json!({
                "scope_id":"default", "id":"rule_live", "requirement_ids":{"add":["req_old"]}
            }))
            .unwrap(),
        )
        .unwrap_err();
    assert_invalid_update(error);
    assert_eq!(store.list_rules(&scope).unwrap(), before);
}

#[test]
#[verifies("rule_live_dependents_have_live_parents", examples)]
fn resolution_update_cannot_add_an_archived_requirement() {
    let (_directory, store, scope) = archived_parent_store();
    store
        .create_resolution(
            serde_json::from_value(resolution_input("res_live", "req_overtime")).unwrap(),
        )
        .unwrap();
    let before = store.list_resolutions(&scope).unwrap();
    let error = store
        .update_resolution(
            serde_json::from_value(json!({
                "scope_id":"default", "id":"res_live", "requirement_ids":{"add":["req_old"]}
            }))
            .unwrap(),
        )
        .unwrap_err();
    assert_invalid_update(error);
    assert_eq!(store.list_resolutions(&scope).unwrap(), before);
}
