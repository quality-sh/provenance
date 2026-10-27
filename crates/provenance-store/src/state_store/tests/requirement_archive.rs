use super::seeded_requirement_store;
use crate::state_store::StateStore;
use crate::write_error::{WriteError, WriteFailure};
use provenance_core::{Requirement, ScopeId, SUPPORTED_SCHEMA_VERSION};
use provenance_macros::verifies;
use serde_json::{json, Value};

pub(super) const COMMIT: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

pub(super) fn child(store: &StateStore, id: &str, parent: &str) {
    store
        .create_requirement(
            serde_json::from_value(json!({
                "scope_id":"default", "id":id, "statement":"The system is safe.",
                "status":"active", "refines":[parent], "depends_on":[], "supersedes":[]
            }))
            .unwrap(),
        )
        .unwrap();
}

pub(super) fn archive(store: &StateStore, id: &str) {
    store
        .update_requirement(
            serde_json::from_value(json!({
                "scope_id":"default", "id":id, "status":"archived",
                "archived_in_commit":{"commit":COMMIT}
            }))
            .unwrap(),
        )
        .unwrap();
}

pub(super) fn requirement(store: &StateStore, scope: &ScopeId, id: &str) -> Requirement {
    store
        .list_requirements(scope)
        .unwrap()
        .into_iter()
        .find(|record| record.id.as_str() == id)
        .unwrap()
}

pub(super) fn assert_invalid_update(error: anyhow::Error) {
    assert!(
        matches!(WriteError(error).safe(), WriteFailure::InvalidUpdate),
        "expected invalid_update"
    );
}

/// Appends one record as a merge of two branches can leave it.
fn append(path: &camino::Utf8Path, record: &Value) {
    let mut contents = std::fs::read_to_string(path).unwrap_or_default();
    contents.push_str(&record.to_string());
    contents.push('\n');
    std::fs::write(path, contents).unwrap();
}

pub(super) fn append_rule(store: &StateStore, scope: &ScopeId, id: &str, parents: &[&str]) {
    append(
        &crate::shards::rules_path(&store.layout, scope),
        &json!({
            "schema_version": SUPPORTED_SCHEMA_VERSION.0, "scope_id": scope.as_str(),
            "id": id, "statement": "The system is safe.", "status": "draft",
            "severity": "medium", "requirement_ids": parents, "resolution_ids": []
        }),
    );
}

pub(super) fn append_requirement(store: &StateStore, scope: &ScopeId, id: &str, parents: &[&str]) {
    append(
        &crate::shards::requirements_path(&store.layout, scope),
        &json!({
            "schema_version": SUPPORTED_SCHEMA_VERSION.0, "scope_id": scope.as_str(),
            "id": id, "statement": "The system is safe.", "status": "active",
            "refines": parents
        }),
    );
}

#[test]
#[verifies("rule_archive_is_terminal_for_each_record", examples)]
#[verifies("rule_archive_records_each_commit", examples)]
fn requirement_archive_stores_commit_and_refuses_live_status() {
    let (_directory, store, scope) = seeded_requirement_store();
    archive(&store, "req_overtime");
    let archived = requirement(&store, &scope, "req_overtime");
    let value = serde_json::to_value(&archived).unwrap();
    assert_eq!(value["status"], "archived");
    assert_eq!(value["archived_in_commit"]["commit"], COMMIT);
    let error = store
        .update_requirement(
            serde_json::from_value(json!({
                "scope_id":scope, "id":"req_overtime", "status":"active"
            }))
            .unwrap(),
        )
        .unwrap_err();
    assert_invalid_update(error);
    assert_eq!(requirement(&store, &scope, "req_overtime"), archived);
}

#[test]
#[verifies("rule_archive_is_terminal_for_each_record", examples)]
fn archived_requirement_refuses_a_changed_commit() {
    let (_directory, store, scope) = seeded_requirement_store();
    archive(&store, "req_overtime");
    let archived = requirement(&store, &scope, "req_overtime");
    let error = store
        .update_requirement(
            serde_json::from_value(json!({
                "scope_id":scope, "id":"req_overtime", "status":"archived",
                "archived_in_commit":{"commit":"b".repeat(40)}
            }))
            .unwrap(),
        )
        .unwrap_err();
    assert_invalid_update(error);
    assert_eq!(requirement(&store, &scope, "req_overtime"), archived);
}

#[test]
#[verifies("rule_archive_records_each_commit", examples)]
fn later_update_to_archived_requirement_does_not_archive_again() {
    let (_directory, store, scope) = seeded_requirement_store();
    archive(&store, "req_overtime");
    append_rule(&store, &scope, "rule_merged", &["req_overtime"]);
    let before = store.list_rules(&scope).unwrap();
    store
        .update_requirement(
            serde_json::from_value(json!({
                "scope_id":scope, "id":"req_overtime", "description":"Old pay rules."
            }))
            .unwrap(),
        )
        .unwrap();
    assert_eq!(store.list_rules(&scope).unwrap(), before);
}

#[test]
#[verifies("rule_archive_cascades_without_surviving_parents", examples)]
#[verifies("rule_archive_completes_cascade_in_one_operation", examples)]
#[verifies("rule_archive_does_not_create_roots", examples)]
#[verifies("rule_live_dependents_have_live_parents", examples)]
#[verifies("rule_archive_keeps_archived_parent_links", examples)]
#[verifies("rule_archive_records_each_commit", examples)]
fn sole_parent_chain_archives_in_one_operation_and_keeps_parents() {
    let (_directory, store, scope) = seeded_requirement_store();
    child(&store, "req_child", "req_overtime");
    child(&store, "req_leaf", "req_child");
    archive(&store, "req_overtime");
    for (id, parent) in [("req_child", "req_overtime"), ("req_leaf", "req_child")] {
        let value = serde_json::to_value(requirement(&store, &scope, id)).unwrap();
        assert_eq!(value["status"], "archived");
        assert_eq!(value["refines"], json!([parent]));
        assert_eq!(value["archived_in_commit"]["commit"], COMMIT);
    }
}

#[test]
#[verifies("rule_archive_completes_cascade_in_one_operation", examples)]
#[verifies("rule_archive_keeps_archived_parent_links", examples)]
#[verifies("rule_archive_records_each_commit", examples)]
fn requirement_archive_includes_topic_and_question() {
    let (_directory, store, scope) = seeded_requirement_store();
    store
        .create_topic(
            serde_json::from_value(json!({
                "scope_id":scope, "id":"topic_child", "requirement_id":"req_overtime",
                "title":"Stored history", "status":"open", "links":[]
            }))
            .unwrap(),
        )
        .unwrap();
    store
        .create_question(
            serde_json::from_value(json!({
                "scope_id":scope, "id":"question_child", "topic_id":"topic_child",
                "question":"Which record applies?", "resolution_method":"research",
                "status":"open", "links":[]
            }))
            .unwrap(),
        )
        .unwrap();
    archive(&store, "req_overtime");
    let topic = store
        .list_topics(&scope)
        .unwrap()
        .into_iter()
        .find(|record| record.id.as_str() == "topic_child")
        .unwrap();
    let question = store
        .list_questions(&scope)
        .unwrap()
        .into_iter()
        .find(|record| record.id.as_str() == "question_child")
        .unwrap();
    let topic: Value = serde_json::to_value(topic).unwrap();
    let question: Value = serde_json::to_value(question).unwrap();
    assert_eq!(topic["status"], "archived");
    assert_eq!(topic["requirement_id"], "req_overtime");
    assert_eq!(question["status"], "archived");
    assert_eq!(question["topic_id"], "topic_child");
    assert_eq!(question["requirement_id"], "req_overtime");
    for record in [topic, question] {
        assert_eq!(record["archived_in_commit"]["commit"], COMMIT);
    }
}
