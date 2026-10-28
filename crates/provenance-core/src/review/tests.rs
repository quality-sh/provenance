use super::ReviewRecord;
use crate::NodeType;
use serde_json::json;

/// Implementation aid: pins the closed read of an enrolled record; no Rule
/// names it.
#[test]
fn closed_record_read_refuses_unknown_fields() {
    let mut value = json!({
        "schema_version": 3,
        "scope_id": "default",
        "id": "source_a",
        "name": "Policy",
        "source_type": "document",
        "url": null
    });
    let record = ReviewRecord::deserialize_closed(NodeType::Source, &value).unwrap();
    assert!(matches!(record, ReviewRecord::Source(_)));
    value["unexpected"] = json!(true);
    let error = ReviewRecord::deserialize_closed(NodeType::Source, &value)
        .unwrap_err()
        .to_string();
    assert!(error.contains("unknown field `unexpected`"), "{error}");
}
