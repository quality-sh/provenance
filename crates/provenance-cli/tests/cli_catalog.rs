use assert_cmd::Command;
use predicates::str::contains;
use serde_json::{json, Value};

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
