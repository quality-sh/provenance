use assert_cmd::Command;
use serde_json::{json, Value};
use std::net::TcpListener;

fn provenance() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
}

fn initialized_repo() -> (tempfile::TempDir, String) {
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

fn json_output(arguments: &[&str]) -> Value {
    let output = provenance().args(arguments).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn publish_fake_host(repo: &str) -> (TcpListener, String) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let path = std::path::Path::new(repo).join(".provenance/cache/review-hosts/default.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        path,
        serde_json::to_vec(&json!({
            "endpoint": endpoint,
            "repositoryId": "local",
            "scope": "default"
        }))
        .unwrap(),
    )
    .unwrap();
    (listener, endpoint)
}

#[test]
fn write_output_explains_how_to_start_a_missing_review_host() {
    let (_directory, repo) = initialized_repo();
    let created = json_output(&[
        "req_link",
        "create",
        "--type",
        "requirement",
        "--repo",
        &repo,
        "--statement",
        "The agent gives the reviewer a link.",
        "--format",
        "json",
    ]);

    assert!(created["data"].get("review_url").is_none());
    assert!(created["meta"]["review"].as_str().unwrap().contains("provenance review"));
}

#[test]
fn write_and_explicit_read_link_to_the_containing_requirement() {
    let (_directory, repo) = initialized_repo();
    json_output(&[
        "req_link",
        "create",
        "--type",
        "requirement",
        "--repo",
        &repo,
        "--statement",
        "The agent gives the reviewer a link.",
        "--format",
        "json",
    ]);
    let (_listener, endpoint) = publish_fake_host(&repo);

    let created = json_output(&[
        "rule_link",
        "create",
        "--type",
        "rule",
        "--repo",
        &repo,
        "--statement",
        "The review output includes a link.",
        "--requirement-id",
        "req_link",
        "--format",
        "json",
    ]);
    let expected = format!("{endpoint}/?root=req_link&focus=rule_link");
    assert_eq!(created["data"]["review_url"], expected);

    let link = json_output(&[
        "rule_link",
        "get",
        "--repo",
        &repo,
        "--review-link",
        "--format",
        "json",
    ]);
    assert_eq!(link["review_url"], expected);
}

#[test]
fn explicit_link_read_explains_how_to_start_the_host() {
    let (_directory, repo) = initialized_repo();
    json_output(&[
        "req_link",
        "create",
        "--type",
        "requirement",
        "--repo",
        &repo,
        "--statement",
        "The agent gives the reviewer a link.",
        "--format",
        "json",
    ]);

    let output = json_output(&[
        "req_link",
        "get",
        "--repo",
        &repo,
        "--review-link",
        "--format",
        "json",
    ]);
    assert!(output["review_url"].is_null());
    assert!(output["message"].as_str().unwrap().contains("provenance review"));
}
