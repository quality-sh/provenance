use crate::{ArchivedStamp, Requirement, Resolution, Rule, Source, Stamp};
use serde_json::json;

#[test]
fn record_stamps_validate_full_hashes_and_rfc3339_times() {
    for length in [40, 64] {
        let stamp = json!({"commit":"a".repeat(length),"at":"2026-09-12T10:00:00+10:00"});
        assert!(serde_json::from_value::<Stamp>(stamp.clone()).is_ok());
        assert!(serde_json::from_value::<ArchivedStamp>(stamp).is_ok());
        assert!(
            serde_json::from_value::<ArchivedStamp>(json!({"commit":"0".repeat(length)})).is_ok()
        );
    }
    for commit in [
        "a".repeat(39),
        "a".repeat(41),
        "g".repeat(40),
        "A".repeat(64),
    ] {
        assert!(serde_json::from_value::<Stamp>(
            json!({"commit":commit,"at":"2026-09-12T00:00:00Z"})
        )
        .is_err());
    }
    for at in ["yesterday", "2026-09-12", "2026-99-99T00:00:00Z"] {
        assert!(serde_json::from_value::<Stamp>(json!({"commit":"a".repeat(40),"at":at})).is_err());
        assert!(
            serde_json::from_value::<ArchivedStamp>(json!({"commit":"a".repeat(40),"at":at}))
                .is_err()
        );
    }
    assert!(serde_json::from_value::<Stamp>(json!({"commit":"a".repeat(40)})).is_err());
}

fn compare<T: serde::de::DeserializeOwned + serde::Serialize + PartialEq + std::fmt::Debug>(
    mut record: serde_json::Value,
    field: &str,
) {
    let before: T = serde_json::from_value(record.clone()).unwrap();
    assert!(serde_json::to_value(&before)
        .unwrap()
        .get("created")
        .is_none());
    record["created"] = json!({"commit":"a".repeat(40),"at":"2026-09-12T00:00:00Z"});
    record["updated"] = json!({"commit":"b".repeat(64),"at":"2026-09-12T01:00:00Z"});
    let stamped: T = serde_json::from_value(record.clone()).unwrap();
    assert_eq!(before, stamped);
    assert_ne!(
        serde_json::to_value(&before).unwrap(),
        serde_json::to_value(&stamped).unwrap()
    );
    record[field] = json!("Changed content");
    assert_ne!(before, serde_json::from_value::<T>(record).unwrap());
}

#[test]
fn equality_ignores_stamps_on_all_four_record_kinds() {
    let base =
        json!({"schema_version":crate::SUPPORTED_SCHEMA_VERSION.0,"scope_id":"default","id":"one"});
    let with = |fields: serde_json::Value| {
        let mut record = base.clone();
        record
            .as_object_mut()
            .unwrap()
            .extend(fields.as_object().unwrap().clone());
        record
    };
    compare::<Source>(
        with(json!({"name":"Policy","source_type":"policy","url":null})),
        "name",
    );
    compare::<Requirement>(
        with(json!({"statement":"Original","status":"active"})),
        "statement",
    );
    compare::<Rule>(
        with(
            json!({"statement":"Original","status":"draft","severity":"medium","requirement_ids":["req_one"]}),
        ),
        "statement",
    );
    compare::<Resolution>(
        with(
            json!({"title":"Original","position":"Adopt","rationale":"Reason","status":"draft","inputs":[],"requirement_ids":["req_one"],"review_on":null}),
        ),
        "title",
    );
}
