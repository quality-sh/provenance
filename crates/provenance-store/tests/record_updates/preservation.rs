use super::support::Fixture;
use serde_json::{json, Value};

fn shard_records(fixture: &Fixture, family: &str, file: &str) -> Vec<Value> {
    let path = fixture
        .dir
        .path()
        .join(format!(".provenance/state/scopes/default/{family}/{file}"));
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn write_shard_records(fixture: &Fixture, family: &str, file: &str, records: &[Value]) {
    let path = fixture
        .dir
        .path()
        .join(format!(".provenance/state/scopes/default/{family}/{file}"));
    let contents = records
        .iter()
        .map(|record| serde_json::to_string(record).unwrap())
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    std::fs::write(path, contents).unwrap();
}

#[tokio::test]
async fn create_preserves_an_unrelated_same_version_record() {
    let fixture = Fixture::new();
    fixture.requirement().await;
    fixture.call("create-resolution", json!({"scope_id":"default","id":"resolution_old","title":"Old decision","position":"Keep the saved record.","rationale":"The record remains available.","status":"draft","requirement_ids":["req_one"],"supersedes":[],"inputs":[]})).await.unwrap();
    let mut records = shard_records(&fixture, "resolutions", "res.jsonl");
    records[0]["superseded_by"] = json!("resolution_future");
    let expected = records[0].clone();
    write_shard_records(&fixture, "resolutions", "res.jsonl", &records);

    fixture.call("create-resolution", json!({"scope_id":"default","id":"resolution_new","title":"New decision","position":"Add another record.","rationale":"The new record is required.","status":"draft","requirement_ids":["req_one"],"supersedes":[],"inputs":[]})).await.unwrap();

    let saved = shard_records(&fixture, "resolutions", "res.jsonl");
    assert_eq!(
        saved.iter().find(|record| record["id"] == "resolution_old"),
        Some(&expected)
    );
}

#[tokio::test]
async fn update_preserves_an_unrelated_record_with_an_alias() {
    let fixture = Fixture::new();
    fixture.source().await;
    fixture.call("create-source", json!({"scope_id":"default","id":"source_two","name":"Second","source_type":"policy","url":null,"supersedes":[]})).await.unwrap();
    let mut records = shard_records(&fixture, "sources", "source.jsonl");
    let first = records
        .iter_mut()
        .find(|record| record["id"] == "source_one")
        .unwrap();
    first["sourceType"] = first
        .as_object_mut()
        .unwrap()
        .remove("source_type")
        .unwrap();
    first["extension"] = json!({"owner":"newer-tool"});
    let expected = first.clone();
    write_shard_records(&fixture, "sources", "source.jsonl", &records);

    fixture
        .call(
            "update-source",
            json!({"scope_id":"default","id":"source_two","name":"Changed"}),
        )
        .await
        .unwrap();

    let saved = shard_records(&fixture, "sources", "source.jsonl");
    assert_eq!(
        saved.iter().find(|record| record["id"] == "source_one"),
        Some(&expected)
    );
}

#[tokio::test]
async fn update_clears_a_known_field_and_preserves_an_unknown_field() {
    let fixture = Fixture::new();
    fixture.source().await;
    let mut records = shard_records(&fixture, "sources", "source.jsonl");
    records[0]["sourceType"] = records[0]
        .as_object_mut()
        .unwrap()
        .remove("source_type")
        .unwrap();
    records[0]["commitPin"] = records[0]
        .as_object_mut()
        .unwrap()
        .remove("commit_pin")
        .unwrap();
    records[0]["extension"] = json!({"owner":"newer-tool"});
    write_shard_records(&fixture, "sources", "source.jsonl", &records);

    fixture.call("update-source", json!({"scope_id":"default","id":"source_one","clear_fields":["reference","commit_pin"]})).await.unwrap();

    let saved = &shard_records(&fixture, "sources", "source.jsonl")[0];
    assert_eq!(saved["extension"], json!({"owner":"newer-tool"}));
    assert_eq!(saved["source_type"], "policy");
    assert!(saved.get("sourceType").is_none());
    assert!(saved.get("reference").is_none());
    assert!(saved.get("commitPin").is_none());
    assert!(saved.get("commit_pin").is_none());
}
