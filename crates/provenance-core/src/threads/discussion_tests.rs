use super::*;
use serde_json::json;

fn discussion() -> Discussion {
    serde_json::from_value(json!({"scope_id":"default","discussion_id":"discussion_a",
        "parent":{"node_type":"requirement","node_id":"req_a"},"thread_id":"thread_a","root_message_id":"message_a",
        "message_ids":["message_a","reply"],"status":"active","version":2,"actor":"ben",
        "outcomes":[{"message_id":"reply","record_kind":"requirement","record_id":"req_a","revision":"abc"}]})).unwrap()
}

/// Implementation aid: pins the Discussion record checks that store reads
/// apply; no Rule names them.
#[test]
fn discussion_record_keeps_its_root_versions_and_outcome_messages() {
    assert!(discussion().validate().is_ok());
    let mut rootless = discussion();
    rootless.message_ids.remove(0);
    assert!(rootless.validate().is_err());
    let mut repeated = discussion();
    repeated.message_ids.push(repeated.root_message_id.clone());
    assert!(repeated.validate().is_err());
    let mut behind = discussion();
    behind.version = 1;
    assert!(behind.validate().is_err());
    let mut foreign = discussion();
    foreign.outcomes[0].message_id = crate::StableId::new("other").unwrap();
    assert!(foreign.validate().is_err());
}
