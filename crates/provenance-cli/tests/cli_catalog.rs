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
            "sources", "create", "--repo", repo, "--id", id, "--name", id,
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
            "verification-runs",
            "begin-verification",
            "--repo",
            repo,
            "--scope",
            "default",
            "--stdin",
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
fn collection_commands_use_resource_addresses_and_v2_envelopes() {
    let (_directory, repo) = init();
    let source = json!({
        "id":"source_catalog", "name":"Catalog source",
        "source_type":"document", "supersedes":[]
    });
    let created = provenance()
        .args(["sources", "create", "--repo", &repo, "--stdin"])
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
        .args(["--repo", &repo, "sources", "list", "--format", "json"])
        .output()
        .unwrap();
    assert!(
        listed.status.success(),
        "{}",
        String::from_utf8_lossy(&listed.stderr)
    );
    let listed: Value = serde_json::from_slice(&listed.stdout).unwrap();
    assert_eq!(listed["data"]["items"][0]["id"], "source_catalog");

    provenance()
        .args(["sources", "--help"])
        .assert()
        .success()
        .stdout(contains("sources list"));
}

#[test]
fn rich_and_scalar_body_inputs_follow_one_rule() {
    let (_directory, repo) = init();
    provenance()
        .args([
            "domains",
            "create",
            "--repo",
            &repo,
            "--id",
            "domain_catalog",
            "--name",
            "Catalog",
        ])
        .assert()
        .success();
    provenance()
        .args([
            "sources",
            "create",
            "--repo",
            &repo,
            "--id",
            "source_bad",
            "--name",
            "Bad",
            "--source-type",
            "document",
            "--supersedes",
            "[]",
        ])
        .assert()
        .failure()
        .stderr(contains("arrays and objects must come from --stdin"));
}

#[test]
fn bodyless_list_filters_are_query_parameters() {
    let (_directory, repo) = init();
    provenance()
        .args([
            "requirements",
            "create",
            "--repo",
            &repo,
            "--id",
            "req_catalog",
            "--statement",
            "The system records verification evidence.",
        ])
        .assert()
        .success();
    for rule in ["rule_catalog_a", "rule_catalog_b"] {
        provenance()
            .args(["rules", "create", "--repo", &repo, "--stdin"])
            .write_stdin(
                json!({
                    "id": rule,
                    "statement": "The system records verification evidence.",
                    "requirement_ids": ["req_catalog"],
                    "resolution_ids": []
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
            "verification-runs",
            "list",
            "--repo",
            &repo,
            "--scope",
            "default",
            "--rule",
            "rule_catalog_b",
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
fn collection_lists_accept_the_returned_cursor() {
    let (_directory, repo) = init();
    create_source(&repo, "source_catalog_a");
    create_source(&repo, "source_catalog_b");

    let first = provenance()
        .args([
            "sources", "list", "--repo", &repo, "--limit", "1", "--format", "json",
        ])
        .output()
        .unwrap();
    assert!(first.status.success());
    let first: Value = serde_json::from_slice(&first.stdout).unwrap();
    let cursor = first["meta"]["next_cursor"].as_str().unwrap();

    let second = provenance()
        .args([
            "sources", "list", "--repo", &repo, "--cursor", cursor, "--format", "json",
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
fn member_reads_return_a_typed_refusal_for_unknown_query_parameters() {
    let (_directory, repo) = init();
    create_source(&repo, "source_catalog_member");

    provenance()
        .args([
            "sources",
            "get",
            "source_catalog_member",
            "--repo",
            &repo,
            "--stray",
            "value",
            "--format",
            "json",
        ])
        .assert()
        .failure()
        .stderr(contains("\"kind\":\"invalid_input\""))
        .stderr(contains("\"field\":\"stray\""));
}

#[test]
fn scalar_flags_follow_the_declared_body_field_type() {
    let (_directory, repo) = init();
    let output = provenance()
        .args([
            "sources",
            "create",
            "--repo",
            &repo,
            "--id",
            "source_numeric_name",
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
