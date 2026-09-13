use super::*;
use serde_json::json;

fn root() -> DiscussionEntry {
    serde_json::from_value(json!({"schema_version":3,"scope_id":"default","id":"event1","parent":{"node_type":"requirement","node_id":"req_a"},
        "thread_id":"thread_a","discussion_id":"discussion_a","root_message_id":"message_a","version":1,"predecessor":null,"status":"active","fact":"started","message_id":"message_a","actor":"ben","request_id":"request1","intent_digest":"digest"})).unwrap()
}

#[test]
fn discussion_chain_preserves_root_and_versions() {
    let root = root();
    assert!(root.validate_after(None).is_ok());
    let mut reply = root.clone();
    reply.id = crate::StableId::new("event2").unwrap();
    reply.predecessor = Some(root.id.clone());
    reply.version = 2;
    reply.fact = DiscussionFact::Replied;
    reply.message_id = Some(crate::StableId::new("reply").unwrap());
    assert!(reply.validate_after(Some(&root)).is_ok());
    for field in [
        "root_message_id",
        "thread_id",
        "discussion_id",
        "scope_id",
        "predecessor",
    ] {
        let mut value = serde_json::to_value(&reply).unwrap();
        value[field] = json!("other");
        let changed: DiscussionEntry = serde_json::from_value(value).unwrap();
        assert!(changed.validate_after(Some(&root)).is_err(), "{field}");
    }
    reply.version = 1;
    assert!(reply.validate_after(Some(&root)).is_err());
}

#[test]
fn resolved_discussion_accepts_only_a_status_transition() {
    let root = root();
    let mut resolved = root.clone();
    resolved.id = crate::StableId::new("event2").unwrap();
    resolved.predecessor = Some(root.id.clone());
    resolved.version = 2;
    resolved.fact = DiscussionFact::StatusChanged;
    resolved.message_id = None;
    resolved.status = DiscussionStatus::Resolved;
    assert!(resolved.validate_after(Some(&root)).is_ok());
    let mut reply = resolved.clone();
    reply.id = crate::StableId::new("event3").unwrap();
    reply.predecessor = Some(resolved.id.clone());
    reply.version = 3;
    reply.fact = DiscussionFact::Replied;
    reply.message_id = Some(crate::StableId::new("reply").unwrap());
    assert!(reply.validate_after(Some(&resolved)).is_err());
    reply.fact = DiscussionFact::StatusChanged;
    reply.message_id = None;
    reply.status = DiscussionStatus::Active;
    assert!(reply.validate_after(Some(&resolved)).is_ok());
}
