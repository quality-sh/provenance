use super::support::Fixture;
use serde_json::{json, Value};
use std::path::PathBuf;

const COMMIT: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const AT: &str = "2026-09-01T00:00:00Z";

fn shard_path(fixture: &Fixture, family: &str, file: &str) -> PathBuf {
    fixture
        .dir
        .path()
        .join(format!(".provenance/state/scopes/default/{family}/{file}"))
}

fn raw_rows(fixture: &Fixture, family: &str, file: &str) -> Vec<String> {
    std::fs::read_to_string(shard_path(fixture, family, file))
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect()
}

fn parsed_rows(fixture: &Fixture, family: &str, file: &str) -> Vec<Value> {
    raw_rows(fixture, family, file)
        .iter()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn write_rows(fixture: &Fixture, family: &str, file: &str, rows: &[String]) {
    std::fs::write(
        shard_path(fixture, family, file),
        format!("{}\n", rows.join("\n")),
    )
    .unwrap();
}

fn padded(value: &Value) -> String {
    format!("  {}  ", serde_json::to_string(value).unwrap())
}

#[tokio::test]
async fn create_keeps_every_unrelated_source_row_byte_for_byte() {
    let fixture = Fixture::new();
    fixture.source().await;
    fixture
        .call(
            "create-source",
            json!({"scope_id":"default","id":"source_two","name":"Second","source_type":"policy","url":null,"supersedes":[]}),
        )
        .await
        .unwrap();
    let mut records = parsed_rows(&fixture, "sources", "source.jsonl");
    records[0]["created"] = json!({"commit":COMMIT,"at":AT});
    records[0]["updated"] = json!({"commit":COMMIT,"at":AT});
    records[0]["sourceType"] = records[0]
        .as_object_mut()
        .unwrap()
        .remove("source_type")
        .unwrap();
    records[0]["extension"] = json!({"owner":"newer-tool","detail":{"level":2}});
    records[1]["extension"] = json!(["second", {"nested":true}]);
    let before = records.iter().map(padded).collect::<Vec<_>>();
    write_rows(&fixture, "sources", "source.jsonl", &before);

    let created = fixture
        .call(
            "create-source",
            json!({"scope_id":"default","id":"source_three","name":"Third","source_type":"policy","url":null,"supersedes":[]}),
        )
        .await
        .unwrap();

    let after = raw_rows(&fixture, "sources", "source.jsonl");
    for expected in before {
        assert!(
            after.contains(&expected),
            "missing untouched row: {expected}"
        );
    }
    assert!(after
        .iter()
        .map(|row| serde_json::from_str::<Value>(row).unwrap())
        .any(|record| record == created));
}

#[tokio::test]
async fn source_reference_edit_keeps_an_unrelated_requirement_exactly() {
    let fixture = Fixture::new();
    fixture.source().await;
    fixture.requirement().await;
    fixture
        .call(
            "create-requirement",
            json!({"scope_id":"default","id":"req_two","statement":"The system reads the record.","status":"active","depends_on":[],"supersedes":[]}),
        )
        .await
        .unwrap();
    let mut records = parsed_rows(&fixture, "requirements", "req.jsonl");
    records[0]["created"] = json!({"commit":COMMIT,"at":AT});
    records[0]["updated"] = json!({"commit":COMMIT,"at":AT});
    records[0]["source_refs"] = json!([{"source_id":"source_one","clause":"section 1"}]);
    records[0]["extension"] = json!({"preserve":true});
    let expected = padded(&records[0]);
    let second = padded(&records[1]);
    write_rows(
        &fixture,
        "requirements",
        "req.jsonl",
        &[expected.clone(), second],
    );

    fixture
        .call(
            "add-source-reference",
            json!({"scope_id":"default","requirement_id":"req_two","source_id":"source_one","clause":"section 2"}),
        )
        .await
        .unwrap();

    assert_eq!(raw_rows(&fixture, "requirements", "req.jsonl")[0], expected);
}

#[tokio::test]
async fn target_edit_keeps_top_level_unknown_data_and_accepts_aliases() {
    let fixture = Fixture::new();
    fixture.source().await;
    let mut record = parsed_rows(&fixture, "sources", "source.jsonl").remove(0);
    record["sourceType"] = record
        .as_object_mut()
        .unwrap()
        .remove("source_type")
        .unwrap();
    record["commitPin"] = record
        .as_object_mut()
        .unwrap()
        .remove("commit_pin")
        .unwrap();
    record["extension"] = json!({"owner":"newer-tool","nested":{"value":7}});
    write_rows(&fixture, "sources", "source.jsonl", &[padded(&record)]);

    let updated = fixture
        .call(
            "update-source",
            json!({"scope_id":"default","id":"source_one","name":"Changed"}),
        )
        .await
        .unwrap();

    let saved = parsed_rows(&fixture, "sources", "source.jsonl").remove(0);
    assert_eq!(updated["name"], "Changed");
    assert_eq!(saved["name"], "Changed");
    assert_eq!(saved["source_type"], "policy");
    assert_eq!(saved["commit_pin"], COMMIT);
    assert_eq!(saved["extension"], record["extension"]);
    assert!(saved.get("sourceType").is_none());
    assert!(saved.get("commitPin").is_none());
}

#[tokio::test]
async fn target_edit_with_nested_unknown_data_is_refused_without_publication() {
    let fixture = Fixture::new();
    fixture.source().await;
    fixture.requirement().await;
    let mut record = parsed_rows(&fixture, "requirements", "req.jsonl").remove(0);
    record["sourceRefs"] =
        json!([{"sourceId":"source_one","clause":"section 1","extension":"keep"}]);
    let original = padded(&record);
    write_rows(
        &fixture,
        "requirements",
        "req.jsonl",
        std::slice::from_ref(&original),
    );

    let error = fixture
        .call(
            "update-requirement",
            json!({"scope_id":"default","id":"req_one","description":"Changed"}),
        )
        .await
        .unwrap_err();

    assert_ne!(error["kind"], "unknown_operation");
    assert_eq!(raw_rows(&fixture, "requirements", "req.jsonl"), [original]);
}
