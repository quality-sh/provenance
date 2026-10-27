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
