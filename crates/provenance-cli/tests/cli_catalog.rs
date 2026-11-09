use assert_cmd::Command;
use predicates::str::contains;
use serde_json::{json, Value};
use std::path::Path;

fn provenance() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
}

fn init() -> (tempfile::TempDir, String) {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_string_lossy().into_owned();
    provenance()
        .args([
            "init",
            "--path",
            &repo,
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();
    (directory, repo)
}

fn create_source(repo: &str, id: &str) {
    provenance()
        .args([
            id, "create", "--type", "source", "--repo", repo, "--name", id,
        ])
        .assert()
        .success();
}

fn create_verification_run(repo: &str, rule: &str, key: &str) {
    let file = format!("tests/{key}.rs");
    let path = Path::new(repo).join(&file);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, "// verifies the rule\n").unwrap();
    provenance()
        .args([
            "api",
            "verification-runs/begin-verification",
            "--method",
            "post",
            "--input",
            "-",
            "--repo",
            repo,
            "--scope",
            "default",
        ])
        .write_stdin(
            json!({
                "rule": rule,
                "key": key,
                "method": "examples",
                "declared_by": "cli-catalog-test",
                "file": file
            })
            .to_string(),
        )
        .assert()
        .success();
}

#[test]
fn api_commands_use_resource_addresses_and_response_envelopes() {
    let (_directory, repo) = init();
    let source = json!({
        "id":"source_catalog", "name":"Catalog source",
        "source_type":"document", "supersedes":[]
    });
    let created = provenance()
        .args([
            "api", "sources", "--method", "post", "--input", "-", "--repo", &repo,
        ])
        .write_stdin(source.to_string())
        .output()
        .unwrap();
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );
    let created: Value = serde_json::from_slice(&created.stdout).unwrap();
    assert_eq!(created["data"]["id"], "source_catalog");
    assert_eq!(created["meta"], json!({}));

    let listed = provenance()
        .args(["--repo", &repo, "api", "sources", "--format", "json"])
        .output()
        .unwrap();
    assert!(
        listed.status.success(),
        "{}",
        String::from_utf8_lossy(&listed.stderr)
    );
    let listed: Value = serde_json::from_slice(&listed.stdout).unwrap();
    assert_eq!(listed["data"]["items"][0]["id"], "source_catalog");
}

#[test]
fn rich_and_scalar_body_inputs_follow_one_rule() {
    let (_directory, repo) = init();
    provenance()
        .args([
            "source_catalog",
            "create",
            "--type",
            "source",
            "--repo",
            &repo,
            "--name",
            "Catalog",
            "--source-type",
            "document",
        ])
        .assert()
        .success();
    provenance()
        .args([
            "source_bad",
            "create",
            "--type",
            "source",
            "--repo",
            &repo,
            "--name",
            "Bad",
            "--source-type",
            "document",
            "--supersedes",
            "[]",
        ])
        .assert()
        .failure()
        .stderr(contains(
            "arrays and objects must come from --stdin or --supersedes-json",
        ));
}

#[test]
fn api_list_filters_are_query_parameters() {
    let (_directory, repo) = init();
    provenance()
        .args([
            "req_catalog",
            "create",
            "--type",
            "requirement",
            "--repo",
            &repo,
            "--statement",
            "The system records verification evidence.",
        ])
        .assert()
        .success();
    for rule in ["rule_catalog_a", "rule_catalog_b"] {
        provenance()
            .args([rule, "create", "--type", "rule", "--stdin", "--repo", &repo])
            .write_stdin(
                json!({
                    "statement": "The system records verification evidence.",
                    "requirement_ids": ["req_catalog"],
                    "resolution_ids": [],
                    "status": "active"
                })
                .to_string(),
            )
            .assert()
            .success();
    }
    create_verification_run(&repo, "rule_catalog_a", "catalog-a");
    create_verification_run(&repo, "rule_catalog_b", "catalog-b");

    let output = provenance()
        .args([
            "api",
            "verification-runs",
            "--repo",
            &repo,
            "--scope",
            "default",
            "--query",
            "rule=rule_catalog_b",
            "--format",
            "json",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(output["data"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(output["data"]["items"][0]["rule_id"], "rule_catalog_b");
}

#[test]
fn api_lists_accept_the_returned_cursor() {
    let (_directory, repo) = init();
    create_source(&repo, "source_catalog_a");
    create_source(&repo, "source_catalog_b");

    let first = provenance()
        .args([
            "api", "sources", "--repo", &repo, "--query", "limit=1", "--format", "json",
        ])
        .output()
        .unwrap();
    assert!(first.status.success());
    let first: Value = serde_json::from_slice(&first.stdout).unwrap();
    let cursor = first["meta"]["next_cursor"].as_str().unwrap();

    let second = provenance()
        .args([
            "api",
            "sources",
            "--repo",
            &repo,
            "--query",
            "limit=1",
            "--query",
            &format!("cursor={cursor}"),
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(
        second.status.success(),
        "{}",
        String::from_utf8_lossy(&second.stderr)
    );
    let second: Value = serde_json::from_slice(&second.stdout).unwrap();
    assert_eq!(second["data"]["items"].as_array().unwrap().len(), 1);
    assert_ne!(
        second["data"]["items"][0]["id"],
        first["data"]["items"][0]["id"]
    );
}

#[test]
fn api_cursors_refuse_a_changed_page_limit() {
    let (_directory, repo) = init();
    create_source(&repo, "source_cursor_a");
    create_source(&repo, "source_cursor_b");
    let first = provenance()
        .args([
            "api", "sources", "--repo", &repo, "--query", "limit=1", "--format", "json",
        ])
        .output()
        .unwrap();
    assert!(first.status.success());
    let first: Value = serde_json::from_slice(&first.stdout).unwrap();
    let cursor = first["meta"]["next_cursor"].as_str().unwrap();

    provenance()
        .args([
            "api",
            "sources",
            "--repo",
            &repo,
            "--query",
            "limit=2",
            "--query",
            &format!("cursor={cursor}"),
            "--format",
            "json",
        ])
        .assert()
        .failure()
        .stderr(contains("cursor_invalid"));
}

#[test]
fn record_updates_and_api_reads_address_one_member() {
    let (_directory, repo) = init();
    create_source(&repo, "source_action_one");
    create_source(&repo, "source_action_two");

    let updated = provenance()
        .args([
            "source_action_one",
            "update",
            "--repo",
            &repo,
            "--name",
            "Changed",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(
        updated.status.success(),
        "{}",
        String::from_utf8_lossy(&updated.stderr)
    );
    let updated: Value = serde_json::from_slice(&updated.stdout).unwrap();
    assert_eq!(updated["data"]["id"], "source_action_one");
    assert_eq!(updated["data"]["name"], "Changed");

    let read = provenance()
        .args([
            "api",
            "sources/source_action_two",
            "--repo",
            &repo,
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(
        read.status.success(),
        "{}",
        String::from_utf8_lossy(&read.stderr)
    );
    let read: Value = serde_json::from_slice(&read.stdout).unwrap();
    assert_eq!(read["data"]["id"], "source_action_two");
}

#[test]
fn api_neighbors_query_filters_relations() {
    let (_directory, repo) = init();
    provenance()
        .args([
            "req_parent",
            "create",
            "--type",
            "requirement",
            "--repo",
            &repo,
            "--statement",
            "The system keeps records.",
        ])
        .assert()
        .success();
    provenance()
        .args([
            "req_filter",
            "create",
            "--type",
            "requirement",
            "--repo",
            &repo,
            "--statement",
            "The system stores records.",
            "--refines-json",
            "[\"req_parent\"]",
        ])
        .assert()
        .success();

    let output = provenance()
        .args([
            "api",
            "requirements/req_filter",
            "--query",
            "query=neighbors",
            "--repo",
            &repo,
            "--query",
            "relations=refines",
            "--query",
            "limit=20",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output: Value = serde_json::from_slice(&output.stdout).unwrap();
    let neighbors = output["data"]["neighbors"].as_array().unwrap();
    assert_ne!(neighbors.as_slice(), [] as [Value; 0]);
    assert!(neighbors.iter().all(|item| item["relation"] == "refines"));
}

#[test]
fn scalar_flags_follow_the_declared_body_field_type() {
    let (_directory, repo) = init();
    let output = provenance()
        .args([
            "source_numeric_name",
            "create",
            "--type",
            "source",
            "--repo",
            &repo,
            "--name",
            "123",
            "--url",
            "true",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(output["data"]["name"], "123");
    assert_eq!(output["data"]["url"], "true");
}

#[path = "cli_catalog/question_method.rs"]
mod question_method;
