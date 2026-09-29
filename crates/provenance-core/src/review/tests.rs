use super::{JournalEntry, RecordSnapshot, ReviewEntry, ReviewRecord, REVIEW_SCHEMA_VERSION};
use crate::NodeType;
use serde_json::{json, Value};

fn legacy_entry() -> Value {
    json!({
        "schema_version": 3,
        "scope_id": "default",
        "requirement_id": "req_a",
        "id": "entry_a",
        "sequence": 1,
        "predecessor": null,
        "revision": "revision_a",
        "prior_revision": null,
        "before": null,
        "after": {"id":"snapshot_a","digest":"sha256:a","bytes":1,"fields":[]},
        "changed_fields": ["statement"],
        "actor": "reviewer",
        "request_id": "request_a",
        "intent_digest": "sha256:b",
        "etag": "sha256:c",
        "outcome": "changed"
    })
}

#[test]
fn requirement_entry_keeps_legacy_json() {
    let bytes = br#"{"schema_version":3,"scope_id":"default","requirement_id":"req_a","id":"entry_a","sequence":1,"predecessor":null,"revision":"revision_a","prior_revision":null,"before":null,"after":{"id":"snapshot_a","digest":"sha256:a","bytes":1,"fields":[]},"changed_fields":["statement"],"actor":"reviewer","request_id":"request_a","intent_digest":"sha256:b","etag":"sha256:c","outcome":"changed"}"#;
    let JournalEntry::Record(entry) = serde_json::from_slice(bytes).unwrap() else {
        panic!("legacy review bytes must select the record journal variant");
    };
    assert_eq!(entry.record_kind, NodeType::Requirement);
    assert_eq!(entry.record_id.as_str(), "req_a");
    assert_eq!(serde_json::to_vec(&entry).unwrap(), bytes.as_slice());
}

#[test]
fn non_requirement_entry_carries_record_address() {
    let mut value = legacy_entry();
    let object = value.as_object_mut().unwrap();
    object.remove("requirement_id");
    object.insert("record_kind".into(), json!("source"));
    object.insert("record_id".into(), json!("source_a"));
    let entry: ReviewEntry = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(entry.record_kind, NodeType::Source);
    assert_eq!(entry.record_id.as_str(), "source_a");
    assert_eq!(serde_json::to_value(entry).unwrap(), value);
}

#[test]
fn requirement_snapshot_keeps_legacy_json() {
    let value = json!({
        "schema_version": 3,
        "record": {
            "schema_version": 3,
            "scope_id": "default",
            "id": "req_a",
            "statement": "The system stores records.",
            "status": "active"
        }
    });
    let snapshot: RecordSnapshot = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(snapshot.record.kind(), NodeType::Requirement);
    assert_eq!(serde_json::to_value(snapshot).unwrap(), value);
}

#[test]
fn source_snapshot_carries_its_kind_and_record() {
    let value = json!({
        "schema_version": REVIEW_SCHEMA_VERSION,
        "record_kind": "source",
        "record": {
            "schema_version": 3,
            "scope_id": "default",
            "id": "source_a",
            "name": "Policy",
            "source_type": "document",
            "url": null
        }
    });
    let snapshot: RecordSnapshot = serde_json::from_value(value.clone()).unwrap();
    assert!(matches!(snapshot.record, ReviewRecord::Source(_)));
    assert_eq!(serde_json::to_value(snapshot).unwrap(), value);
}

#[test]
fn source_snapshot_refuses_unknown_record_fields() {
    let value = json!({
        "schema_version": REVIEW_SCHEMA_VERSION,
        "record_kind": "source",
        "record": {
            "schema_version": 3,
            "scope_id": "default",
            "id": "source_a",
            "name": "Policy",
            "source_type": "document",
            "url": null,
            "unexpected": true
        }
    });

    let error = serde_json::from_value::<RecordSnapshot>(value)
        .unwrap_err()
        .to_string();
    assert!(error.contains("unknown field `unexpected`"), "{error}");
}
