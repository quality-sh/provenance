#[allow(dead_code)]
mod review_support;
use provenance_core::StableId;
use provenance_macros::verifies;
use provenance_store::state_store::StateStore;
use review_support::{fixture, scope};
use serde_json::{json, Value};

fn sid(id: &str) -> StableId { StableId::new(id).unwrap() }
fn create(store: &StateStore, id: &str) {
    store.create_requirement(serde_json::from_value(json!({
        "scope_id":"default", "id":id, "statement":"The system stores records.",
        "status":"discovery", "depends_on":[], "supersedes":[]
    })).unwrap()).unwrap();
}
fn read(store: &StateStore, id: &str) -> Value {
    serde_json::to_value(store.list_requirements(&scope()).unwrap().into_iter()
        .find(|r| r.id == sid(id)).unwrap()).unwrap()
}

#[test]
#[verifies("rule_requirement_accepts_multiple_parents", examples)]
fn adding_a_second_parent_keeps_the_diamond_and_refuses_a_cycle() {
    let (_temp, store) = fixture();
    for id in ["req_p", "req_b", "req_child"] { create(&store, id); }
    store.set_requirement_refines(&scope(), &sid("req_child"), sid("req_a")).unwrap();
    let child = read(&store, "req_child");
    store.set_requirement_refines(&scope(), &sid("req_a"), sid("req_p")).unwrap();
    store.set_requirement_refines(&scope(), &sid("req_a"), sid("req_b")).unwrap();
    let shared = read(&store, "req_a");
    assert_eq!(shared["refines"], json!(["req_b", "req_p"]));
    assert_eq!(shared["id"], "req_a");
    assert_eq!(read(&store, "req_child"), child);
    assert!(store.set_requirement_refines(&scope(), &sid("req_b"), sid("req_child")).is_err());
    assert_eq!(read(&store, "req_a"), shared);
}

#[test]
#[verifies("rule_supersession_preserves_unselected_children", examples)]
#[verifies("rule_supersession_does_not_archive_superseded_record", examples)]
fn supersession_changes_no_child_or_parent_status() {
    let (_temp, store) = fixture();
    for id in ["req_p", "req_b"] { create(&store, id); }
    store.set_requirement_refines(&scope(), &sid("req_a"), sid("req_p")).unwrap();
    let child = read(&store, "req_a");
    let parent = read(&store, "req_p");
    store.add_requirement_supersedes(&scope(), &sid("req_b"), sid("req_p")).unwrap();
    assert_eq!(read(&store, "req_a"), child);
    assert_eq!(read(&store, "req_p"), parent);
}

#[test]
#[verifies("rule_shared_requirement_keeps_its_rules", examples)]
fn parents_share_one_requirement_and_all_its_rules() {
    let (_temp, store) = fixture();
    for id in ["req_p", "req_b"] { create(&store, id); }
    store.create_rule(serde_json::from_value(json!({
        "scope_id":"default", "id":"rule_shared", "statement":"The system stores each record.",
        "status":"draft", "severity":"high", "requirement_ids":["req_a"], "resolution_ids":[]
    })).unwrap()).unwrap();
    let rules = store.list_rules(&scope()).unwrap();
    store.set_requirement_refines(&scope(), &sid("req_a"), sid("req_p")).unwrap();
    store.set_requirement_refines(&scope(), &sid("req_a"), sid("req_b")).unwrap();
    assert_eq!(read(&store, "req_a")["refines"], json!(["req_b", "req_p"]));
    assert_eq!(store.list_rules(&scope()).unwrap(), rules);
    assert_eq!(store.list_requirements(&scope()).unwrap().len(), 3);
}

fn edit_parents(store: &StateStore, relations: Value) -> anyhow::Result<provenance_core::review::RequirementEditState> {
    store.save_requirement(serde_json::from_value(json!({
        "actor":"author", "expected_etag":store.requirement_edit_state(&scope(), &sid("req_a"))?.etag,
        "update":{"scope_id":"default", "id":"req_a"},
        "relationships":{"refines":relations}
    }))?)
}

#[test]
#[verifies("rule_move_replaces_only_selected_parent", examples)]
#[verifies("rule_move_preserves_identity_and_descendants", examples)]
#[verifies("rule_supersede_swaps_selected_child_parent", examples)]
fn replacement_keeps_other_parents_and_failed_second_steps_keep_the_new_parent() {
    let (_temp, store) = fixture();
    for id in ["req_p", "req_q", "req_b", "req_child"] { create(&store, id); }
    store.set_requirement_refines(&scope(), &sid("req_child"), sid("req_a")).unwrap();
    let child = read(&store, "req_child");
    edit_parents(&store, json!(["req_p", "req_q"])).unwrap();
    let before = read(&store, "req_a");
    edit_parents(&store, json!({"add":["req_b"]})).unwrap();
    assert!(edit_parents(&store, json!({"remove":["req_missing"]})).is_err());
    assert_eq!(read(&store, "req_a")["refines"], json!(["req_b", "req_p", "req_q"]));
    for _ in 0..2 {
        edit_parents(&store, json!({"add":["req_b"]})).unwrap();
        edit_parents(&store, json!({"remove":["req_p"]})).unwrap();
        assert_eq!(read(&store, "req_a")["refines"], json!(["req_b", "req_q"]));
    }
    let after = read(&store, "req_a");
    for field in ["id", "statement", "status", "description"] { assert_eq!(after[field], before[field]); }
    assert_eq!(read(&store, "req_child"), child);
    assert!(edit_parents(&store, json!({"add":["req_child"]})).is_err());
    assert!(edit_parents(&store, json!({"add":["req_missing"]})).is_err());
    assert_eq!(read(&store, "req_a"), after);
    edit_parents(&store, json!(["req_p"])).unwrap();
    edit_parents(&store, json!({"add":["req_b"]})).unwrap();
    assert_eq!(read(&store, "req_a")["refines"], json!(["req_b", "req_p"]));
    edit_parents(&store, json!({"remove":["req_p"]})).unwrap();
    assert_eq!(read(&store, "req_a")["refines"], json!(["req_b"]));
}
