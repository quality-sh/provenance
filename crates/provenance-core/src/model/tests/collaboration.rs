use super::collaboration::Thread;

#[test]
fn resolved_threads_roundtrip_as_v1_state() {
    let thread = serde_json::json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
        "scope_id": "default",
        "id": "thread_rule_rule_sah_001",
        "parent": {
            "node_type": "rule",
            "node_id": "rule_sah_001"
        },
        "status": "resolved",
        "created_at": 1
    });

    let thread: Thread = serde_json::from_value(thread).unwrap();
    let thread = serde_json::to_value(thread).unwrap();

    assert_eq!(thread["status"], "resolved");
}
