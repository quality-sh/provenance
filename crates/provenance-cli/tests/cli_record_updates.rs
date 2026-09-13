use assert_cmd::Command;
use serde_json::Value;

#[test]
fn source_update_uses_the_local_writer_and_explicit_clears() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().to_str().unwrap();
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "init",
            "--path",
            repo,
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "sources",
            "create",
            "--repo",
            repo,
            "--scope",
            "default",
            "--id",
            "source_one",
            "--name",
            "Policy",
            "--url",
            "https://old.example",
        ])
        .assert()
        .success();
    let output = Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "sources",
            "update",
            "--repo",
            repo,
            "--scope",
            "default",
            "--id",
            "source_one",
            "--fields-json",
            r#"{"reference":"section 2","clear_fields":["url"]}"#,
            "--format",
            "json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let result: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(result["id"], "source_one");
    assert_eq!(result["name"], "Policy");
    assert_eq!(result["reference"], "section 2");
    assert!(result["url"].is_null());
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "sources",
            "update",
            "--repo",
            repo,
            "--scope",
            "default",
            "--id",
            "source_one",
            "--fields-json",
            r#"{"id":"source_other","name":"Wrong"}"#,
        ])
        .assert()
        .failure();
}

#[test]
fn every_descriptive_update_command_uses_the_local_catalog() {
    use serde_json::json;
    let dir = descriptive_records();
    let repo = dir.path().to_str().unwrap();
    for (group, id, patch, field, expected) in [
        (
            "requirements",
            "req_one",
            json!({"description":"Changed"}),
            "description",
            json!("Changed"),
        ),
        (
            "rules",
            "rule_one",
            json!({"name":"Changed","status":"deprecated"}),
            "name",
            json!("Changed"),
        ),
        (
            "resolutions",
            "resolution_one",
            json!({"status":"approved","approved_by":"reviewer","approved_at":1234}),
            "approved_at",
            json!(1234),
        ),
        (
            "domains",
            "domain_one",
            json!({"name":"Changed"}),
            "name",
            json!("Changed"),
        ),
        (
            "boundaries",
            "boundary_one",
            json!({"statement":"Changed"}),
            "statement",
            json!("Changed"),
        ),
        (
            "topics",
            "topic_one",
            json!({"title":"Changed"}),
            "title",
            json!("Changed"),
        ),
        (
            "questions",
            "question_one",
            json!({"question":"Changed?"}),
            "question",
            json!("Changed?"),
        ),
    ] {
        let patch_path = dir.path().join("patch.json");
        std::fs::write(&patch_path, serde_json::to_vec(&patch).unwrap()).unwrap();
        let fields = format!("@{}", patch_path.display());
        let output = Command::cargo_bin("provenance")
            .unwrap()
            .args([
                group,
                "update",
                "--repo",
                repo,
                "--scope",
                "default",
                "--id",
                id,
                "--fields-json",
                &fields,
                "--format",
                "json",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let updated: Value = serde_json::from_slice(&output).unwrap();
        assert_eq!(updated["id"], id);
        assert_eq!(updated[field], expected);
    }
}

fn descriptive_records() -> tempfile::TempDir {
    use provenance_store::{layout::ProvenanceLayout, state_store::StateStore};
    use serde_json::json;
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().to_str().unwrap();
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "init",
            "--path",
            repo,
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();
    let store = StateStore::new(ProvenanceLayout::new(repo));
    store.create_requirement(serde_json::from_value(json!({"scope_id":"default","id":"req_one","statement":"The system saves the record.","status":"active","depends_on":[],"supersedes":[]})).unwrap()).unwrap();
    store.create_rule(serde_json::from_value(json!({"scope_id":"default","id":"rule_one","statement":"The system saves the record.","status":"draft","severity":"medium","requirement_ids":["req_one"],"resolution_ids":[]})).unwrap()).unwrap();
    store.create_resolution(serde_json::from_value(json!({"scope_id":"default","id":"resolution_one","title":"Decision","position":"Use the saved record.","rationale":"The record is available.","status":"draft","requirement_ids":["req_one"],"supersedes":[],"inputs":[]})).unwrap()).unwrap();
    store
        .create_domain(
            serde_json::from_value(json!({"scope_id":"default","id":"domain_one","name":"Domain"}))
                .unwrap(),
        )
        .unwrap();
    store.create_boundary(serde_json::from_value(json!({"scope_id":"default","id":"boundary_one","requirement_id":"req_one","statement":"Limit"})).unwrap()).unwrap();
    store.create_topic(serde_json::from_value(json!({"scope_id":"default","id":"topic_one","requirement_id":"req_one","title":"Topic","status":"open","links":[]})).unwrap()).unwrap();
    store.create_question(serde_json::from_value(json!({"scope_id":"default","id":"question_one","topic_id":"topic_one","question":"Which record?","resolution_method":"research","status":"open","links":[]})).unwrap()).unwrap();
    dir
}
