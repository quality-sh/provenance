use assert_cmd::Command;
use serde_json::Value;

fn run(args: &[&str]) -> std::process::Output {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance")).args(args).output().unwrap()
}

fn success(args: &[&str]) -> std::process::Output {
    let output = run(args);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    output
}

fn json(args: &[&str]) -> Value {
    serde_json::from_slice(&success(args).stdout).unwrap()
}

#[test]
fn cli_discussion_actions_use_one_scope_and_preserve_receipt_identity() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap();
    success(&["init", "--path", repo, "--scope", "default", "--path-prefix", "."]);
    success(&["requirements", "create", "--repo", repo, "--id", "req_a", "--statement", "A discussion has one parent."]);

    let start = json(&["req_a", "discuss", "--repo", repo, "--body", "Opening text", "--format", "json"]);
    assert_eq!(start["kind"], "written");
    assert_eq!(start["receipt"]["version"], 1);
    let id = start["receipt"]["discussion_id"].as_str().unwrap();
    let scope = json(&["discussions", "--repo", repo, "--format", "json"]);
    assert_eq!(scope["result"]["entries"][0]["discussion_id"], id);
    let parent = json(&["req_a", "discussions", "--repo", repo, "--format", "json"]);
    assert_eq!(parent["result"]["entries"][0]["discussion_id"], id);
    let conversation = json(&["discussions", id, "get", "--repo", repo, "--format", "json"]);
    assert_eq!(conversation["result"]["head"]["version"], 1);
    let reply = json(&[id, "reply", "--repo", repo, "--body", "Second message", "--expected-version", "1", "--format", "json"]);
    assert_eq!(reply["receipt"]["request_id"].is_string(), true);
    assert_eq!(reply["receipt"]["version"], 2);
    let get = json(&["req_a", "get", "--repo", repo, "--format", "json"]);
    assert_eq!(get["record"]["id"], "req_a");
}
