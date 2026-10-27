use super::seeded_requirement_store;
use provenance_macros::verifies;
use serde_json::json;

#[test]
#[verifies("rule_archive_is_terminal_for_each_record", examples)]
#[verifies("rule_archive_records_each_commit", examples)]
fn requirement_archive_stores_commit_and_refuses_live_status() {
    let (_directory, store, scope) = seeded_requirement_store();
    let archived = store.update_requirement(serde_json::from_value(json!({
        "scope_id":scope, "id":"req_overtime", "status":"archived",
        "archived_in_commit":{"commit":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}
    })).unwrap()).unwrap();
    let value = serde_json::to_value(archived).unwrap();
    assert_eq!(value["status"], "archived");
    assert_eq!(value["archived_in_commit"]["commit"], "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    let error = store.update_requirement(serde_json::from_value(json!({
        "scope_id":scope, "id":"req_overtime", "status":"active"
    })).unwrap()).unwrap_err();
    assert!(format!("{error:#}").contains("terminal"));
}

fn child(store: &crate::state_store::StateStore, id: &str, parent: &str) {
    store.create_requirement(serde_json::from_value(json!({
        "scope_id":"default", "id":id, "statement":"The service keeps a record.",
        "status":"active", "refines":parent, "depends_on":[], "supersedes":[]
    })).unwrap()).unwrap();
}

fn archive(store: &crate::state_store::StateStore, id: &str) {
    store.update_requirement(serde_json::from_value(json!({
        "scope_id":"default", "id":id, "status":"archived",
        "archived_in_commit":{"commit":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}
    })).unwrap()).unwrap();
}

#[test]
#[verifies("rule_archive_cascades_without_surviving_parents", examples)]
#[verifies("rule_archive_completes_cascade_in_one_operation", examples)]
#[verifies("rule_archive_does_not_create_roots", examples)]
#[verifies("rule_live_dependents_have_live_parents", examples)]
#[verifies("rule_archive_keeps_archived_parent_links", examples)]
fn sole_parent_chain_archives_in_one_operation_and_keeps_parents() {
    let (_directory, store, scope) = seeded_requirement_store();
    child(&store, "req_child", "req_overtime");
    child(&store, "req_leaf", "req_child");
    archive(&store, "req_overtime");
    let records = store.list_requirements(&scope).unwrap();
    for (id, parent) in [("req_child", "req_overtime"), ("req_leaf", "req_child")] {
        let record = records.iter().find(|record| record.id.as_str() == id).unwrap();
        let value = serde_json::to_value(record).unwrap();
        assert_eq!(value["status"], "archived");
        assert_eq!(value["refines"], parent);
        assert_eq!(value["archived_in_commit"]["commit"], "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    }
}

#[test]
#[verifies("rule_archive_cascades_without_surviving_parents", examples)]
#[verifies("rule_archive_keeps_archived_parent_links", examples)]
#[verifies("rule_archive_preserves_surviving_children", examples)]
#[verifies("rule_archive_prunes_only_parent_edge", examples)]
fn diamond_archives_only_when_both_owning_parents_enter_cascade() {
    for shared_branch in [true, false] {
        let (_directory, store, scope) = seeded_requirement_store();
        child(&store, "req_left", "req_overtime");
        child(&store, "req_right", if shared_branch { "req_overtime" } else { "req_left" });
        let parents = if shared_branch { vec!["req_left", "req_right"] } else { vec!["req_left", "req_overtime"] };
        store.create_rule(serde_json::from_value(json!({
            "scope_id":"default", "id":"rule_shared", "statement":"The service keeps a record.",
            "status":"active", "severity":"medium", "requirement_ids":parents, "resolution_ids":[]
        })).unwrap()).unwrap();
        let before = store.list_rules(&scope).unwrap().pop().unwrap();
        archive(&store, if shared_branch { "req_overtime" } else { "req_left" });
        let after = store.list_rules(&scope).unwrap().pop().unwrap();
        let mut before = serde_json::to_value(before).unwrap();
        let mut after = serde_json::to_value(after).unwrap();
        if shared_branch {
            assert_eq!(after["status"], "archived");
            assert_eq!(after["requirement_ids"], before["requirement_ids"]);
            assert!(after["archived_in_commit"]["commit"].is_string());
        } else {
            assert_eq!(after["requirement_ids"], json!(["req_overtime"]));
            for key in ["created", "updated", "requirement_ids"] {
                before.as_object_mut().unwrap().remove(key);
                after.as_object_mut().unwrap().remove(key);
            }
            assert_eq!(after, before);
        }
    }
}
