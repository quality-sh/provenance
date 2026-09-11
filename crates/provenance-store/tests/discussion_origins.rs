#[allow(dead_code)]
mod review_support;
use review_support::*;
use serde_json::json;

#[test]
fn requirement_creation_refuses_a_missing_message_origin_before_publication() {
    let (_temp, store) = fixture();
    let before = store.list_requirements(&scope()).unwrap();
    let result = store.create_requirement(
        serde_json::from_value(json!({
            "scope_id":"default", "id":"req_new", "statement":"The system retains evidence.",
            "status":"discovery", "depends_on":[], "supersedes":[],
            "origin_thread":"missing_thread", "origin_message":"missing_message"
        }))
        .unwrap(),
    );
    assert!(result.is_err(), "a missing origin must refuse creation");
    assert_eq!(before, store.list_requirements(&scope()).unwrap());
}

#[test]
fn old_creation_cannot_omit_an_addressed_outcome() {
    let (_temp, store) = fixture();
    let entry = store.write_discussion(serde_json::from_value(json!({"scope_id":"default","parent":{"node_type":"requirement","node_id":"req_a"},
        "request_id":"root","actor":"ben","action":{"kind":"start","role":"user","body":"A concern"}})).unwrap()).unwrap();
    assert!(store.create_requirement(serde_json::from_value(json!({
        "scope_id":"default","id":"req_new","statement":"The system retains evidence.","status":"discovery","depends_on":[],"supersedes":[],
        "origin_thread":entry.thread_id,"origin_message":entry.message_id
    })).unwrap()).is_err());
}
