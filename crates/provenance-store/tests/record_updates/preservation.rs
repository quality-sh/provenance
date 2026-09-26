use super::support::Fixture;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

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

/// Every untouched row must survive one write with its exact raw bytes, and
/// the shard must gain exactly the one new row.
fn assert_rows_preserved(after: &[String], untouched: &[String], created: &Value) {
    assert_eq!(after.len(), untouched.len() + 1, "{after:?}");
    for expected in untouched {
        let count = after.iter().filter(|row| *row == expected).count();
        assert_eq!(
            count, 1,
            "untouched row must survive exactly once: {expected}"
        );
    }
    let created_row = after
        .iter()
        .find(|row| !untouched.contains(row))
        .expect("the new row");
    assert_eq!(
        serde_json::from_str::<Value>(created_row).unwrap(),
        *created
    );
}

fn state_snapshot(fixture: &Fixture) -> BTreeMap<PathBuf, Vec<u8>> {
    let root = fixture.dir.path().join(".provenance/state");
    let mut files = BTreeMap::new();
    collect_files(&root, &root, &mut files);
    files
}

fn collect_files(root: &Path, current: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
    let mut entries = std::fs::read_dir(current)
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    entries.sort_by_key(std::fs::DirEntry::path);
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            collect_files(root, &path, files);
        } else {
            files.insert(
                path.strip_prefix(root).unwrap().to_owned(),
                std::fs::read(path).unwrap(),
            );
        }
    }
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

    assert_rows_preserved(
        &raw_rows(&fixture, "sources", "source.jsonl"),
        &before,
        &created,
    );
}

#[tokio::test]
async fn create_keeps_a_row_with_repeated_unknown_members_exactly() {
    let fixture = Fixture::new();
    fixture.source().await;
    let mut record = parsed_rows(&fixture, "sources", "source.jsonl").remove(0);
    record["created"] = json!({"commit":COMMIT,"at":AT});
    record["updated"] = json!({"commit":COMMIT,"at":AT});
    let mut raw = serde_json::to_string(&record).unwrap();
    raw.pop();
    raw.push_str(",\"extension\":{\"tool\":\"older\"},\"extension\":{\"tool\":\"newer\"}}");
    let expected = format!("  {raw}  ");
    write_rows(
        &fixture,
        "sources",
        "source.jsonl",
        std::slice::from_ref(&expected),
    );

    let created = fixture
        .call(
            "create-source",
            json!({"scope_id":"default","id":"source_two","name":"Second","source_type":"policy","url":null,"supersedes":[]}),
        )
        .await
        .unwrap();

    assert_rows_preserved(
        &raw_rows(&fixture, "sources", "source.jsonl"),
        &[expected],
        &created,
    );
}

#[tokio::test]
async fn resolution_create_keeps_unrelated_superseded_by_exactly() {
    let fixture = Fixture::new();
    fixture.requirement().await;
    let old = json!({
        "scope_id": "default",
        "id": "resolution_old",
        "title": "Old decision",
        "position": "Keep the saved record.",
        "rationale": "The record remains available.",
        "status": "draft",
        "requirement_ids": ["req_one"],
        "supersedes": [],
        "inputs": []
    });
    fixture.call("create-resolution", old).await.unwrap();
    let mut record = parsed_rows(&fixture, "resolutions", "res.jsonl").remove(0);
    record["superseded_by"] = json!("resolution_future");
    let expected = padded(&record);
    write_rows(
        &fixture,
        "resolutions",
        "res.jsonl",
        std::slice::from_ref(&expected),
    );

    let new = json!({
        "scope_id": "default",
        "id": "resolution_new",
        "title": "New decision",
        "position": "Add another record.",
        "rationale": "The new record is required.",
        "status": "draft",
        "requirement_ids": ["req_one"],
        "supersedes": [],
        "inputs": []
    });
    fixture.call("create-resolution", new).await.unwrap();

    let after = raw_rows(&fixture, "resolutions", "res.jsonl");
    assert_eq!(after.len(), 2);
    assert_eq!(after.iter().filter(|row| *row == &expected).count(), 1);
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
    records[0]["schema_version"] = json!(2);
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
        .store
        .add_source_reference(
            serde_json::from_value(
                json!({"scope_id":"default","requirement_id":"req_two","source_id":"source_one","clause":"section 2"}),
            )
            .unwrap(),
        )
        .unwrap();

    let after = raw_rows(&fixture, "requirements", "req.jsonl");
    assert_eq!(after.len(), 2);
    assert_eq!(after[0], expected);
    assert_eq!(
        serde_json::from_str::<Value>(&after[1]).unwrap()["id"],
        "req_two"
    );
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
    fixture
        .call(
            "create-boundary",
            json!({"scope_id":"default","id":"boundary_one","requirement_id":"req_one","statement":"Limit"}),
        )
        .await
        .unwrap();
    let mut record = parsed_rows(&fixture, "boundaries", "boundary.jsonl").remove(0);
    record["created"] = json!({"commit":COMMIT,"at":AT});
    // The same edit without the nested unknown must go through, so the
    // refusal below is about the nested field and not the row layout.
    let mut clean = record.clone();
    clean["sourceRef"] = json!({"sourceId":"source_one","clause":"section 1"});
    write_rows(&fixture, "boundaries", "boundary.jsonl", &[padded(&clean)]);
    fixture
        .call(
            "update-boundary",
            json!({"scope_id":"default","id":"boundary_one","statement":"Renamed"}),
        )
        .await
        .unwrap();

    let mut loaded = parsed_rows(&fixture, "boundaries", "boundary.jsonl");
    assert_eq!(loaded[0]["statement"], "Renamed");
    loaded[0]["source_ref"]["extension"] = json!("keep");
    let original = padded(&loaded[0]);
    write_rows(
        &fixture,
        "boundaries",
        "boundary.jsonl",
        std::slice::from_ref(&original),
    );
    let before = state_snapshot(&fixture);

    let error = fixture
        .call(
            "update-boundary",
            json!({"scope_id":"default","id":"boundary_one","statement":"Renamed twice"}),
        )
        .await
        .unwrap_err();

    assert_ne!(error["kind"], "unknown_operation");
    assert_eq!(
        raw_rows(&fixture, "boundaries", "boundary.jsonl"),
        vec![original]
    );
    assert_eq!(state_snapshot(&fixture), before);
}

#[tokio::test]
async fn updating_a_row_with_repeated_unknown_members_is_refused_without_publication() {
    let fixture = Fixture::new();
    fixture.source().await;
    let mut record = parsed_rows(&fixture, "sources", "source.jsonl").remove(0);
    record["created"] = json!({"commit":COMMIT,"at":AT});
    record["updated"] = json!({"commit":COMMIT,"at":AT});
    let mut raw = serde_json::to_string(&record).unwrap();
    raw.pop();
    raw.push_str(",\"extension\":{\"tool\":\"older\"},\"extension\":{\"tool\":\"newer\"}}");
    let row = format!("  {raw}  ");
    write_rows(
        &fixture,
        "sources",
        "source.jsonl",
        std::slice::from_ref(&row),
    );
    let before = state_snapshot(&fixture);

    let error = fixture
        .call(
            "update-source",
            json!({"scope_id":"default","id":"source_one","name":"Changed"}),
        )
        .await
        .unwrap_err();

    assert_ne!(error["kind"], "unknown_operation");
    assert_eq!(raw_rows(&fixture, "sources", "source.jsonl"), vec![row]);
    assert_eq!(state_snapshot(&fixture), before);
}
