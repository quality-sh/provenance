#[path = "review_host_support/mod.rs"]
mod review_host_support;

use review_host_support::{repository_with_actors, request, response, start, Host};
use serde_json::{json, Value};
use std::{io::Write as _, path::Path};

fn create_resolution(root: &Path, owner: Option<&str>) {
    let output = std::process::Command::new(assert_cmd::cargo::cargo_bin("provenance"))
        .args([
            "requirements",
            "create",
            "--repo",
            root.to_str().unwrap(),
            "--id",
            "req_reject",
            "--statement",
            "The system stores each result.",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut child = std::process::Command::new(assert_cmd::cargo::cargo_bin("provenance"))
        .args([
            "resolutions",
            "create",
            "--repo",
            root.to_str().unwrap(),
            "--stdin",
            "--format",
            "json",
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(
            json!({
                "id":"res_reject", "title":"Keep the result",
                "position":"The system stores each result.",
                "rationale":"The result must remain available.",
                "status":"proposed", "made_by":owner,
                "requirement_ids":["req_reject"]
            })
            .to_string()
            .as_bytes(),
        )
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn post(host: &Host, path: &str, data: &Value) -> (u16, Value) {
    let result = response(
        request(host, "POST", path, true)
            .set("Content-Type", "application/json")
            .send_string(&json!({"data":data}).to_string()),
    );
    let status = result.status();
    (
        status,
        serde_json::from_str(&result.into_string().unwrap()).unwrap(),
    )
}

fn read(host: &Host) -> Value {
    let result = request(host, "GET", "/resolutions/res_reject", true)
        .call()
        .unwrap();
    serde_json::from_str::<Value>(&result.into_string().unwrap()).unwrap()["data"].clone()
}

fn submission(owner: Option<&str>) -> Value {
    json!({
        "actor":"shaping_agent", "declared_by":owner, "title":"Keep the result",
        "summary":"The system stores each result.", "source_ids":[],
        "evidence_references":[], "builds_on":[]
    })
}

fn submit(host: &Host, owner: Option<&str>) -> (u16, Value) {
    let data = submission(owner);
    let result = post(host, "/resolutions/res_reject/submit", &data);
    println!(
        "POST /resolutions/res_reject/submit {data}\nHTTP {} {}",
        result.0, result.1
    );
    result
}

/// Implementation aid: reproduce the owner's submission failure through the real host.
#[test]
fn resolution_owner_can_submit_through_the_host() {
    let repo = repository_with_actors(&["reviewer"]);
    create_resolution(repo.path(), Some("shaping_agent"));
    let host = start(repo.path());
    let before = read(&host);
    let (status, result) = submit(&host, Some("shaping_agent"));
    assert_eq!(status, 200, "{result}");
    let after = read(&host);
    assert_eq!(
        after["decision"]["pending"]["proposal_id"],
        result["data"]["proposal_id"]
    );
    for field in ["id", "status", "position", "rationale", "made_by", "edit"] {
        assert_eq!(after[field], before[field], "{field}");
    }
}

/// Implementation aid: a wrong submission owner must not publish review evidence.
#[test]
fn wrong_resolution_submission_owner_leaves_the_record_unchanged() {
    let repo = repository_with_actors(&["reviewer"]);
    create_resolution(repo.path(), Some("shaping_agent"));
    let host = start(repo.path());
    let before = read(&host);
    let (status, result) = submit(&host, Some("another_agent"));
    assert_eq!(status, 409, "{result}");
    assert_eq!(result["error"]["kind"], "record_ownership_conflict");
    assert_eq!(read(&host), before);
}

fn pending(host: &Host) -> String {
    let (status, result) = submit(host, Some("shaping_agent"));
    assert_eq!(status, 200, "{result}");
    result["data"]["proposal_id"].as_str().unwrap().to_owned()
}

/// Implementation aid: the same owner check applies to withdrawal through the real host.
#[test]
fn resolution_owner_can_withdraw_through_the_host() {
    let repo = repository_with_actors(&["reviewer"]);
    create_resolution(repo.path(), Some("shaping_agent"));
    let host = start(repo.path());
    let proposal = pending(&host);
    let before = read(&host);
    let path = format!("/resolutions/res_reject/submissions/{proposal}/withdraw");
    let data = json!({"actor":"shaping_agent", "declared_by":"shaping_agent", "reason":"Revise the result."});
    let (status, result) = post(&host, &path, &data);
    println!("POST {path} {data}\nHTTP {status} {result}");
    assert_eq!(status, 200, "{result}");
    let after = read(&host);
    assert!(after["decision"]["pending"].is_null());
    assert_eq!(after["decision"]["withdrawn"], json!([proposal]));
    for field in ["id", "status", "position", "rationale", "made_by", "edit"] {
        assert_eq!(after[field], before[field], "{field}");
    }
}

/// Implementation aid: a wrong withdrawal owner must leave the submission pending.
#[test]
fn wrong_resolution_withdrawal_owner_leaves_the_submission_pending() {
    let repo = repository_with_actors(&["reviewer"]);
    create_resolution(repo.path(), Some("shaping_agent"));
    let host = start(repo.path());
    let proposal = pending(&host);
    let before = read(&host);
    let (status, result) = post(
        &host,
        &format!("/resolutions/res_reject/submissions/{proposal}/withdraw"),
        &json!({"actor":"shaping_agent", "declared_by":"another_agent", "reason":"Revise the result."}),
    );
    assert_eq!(status, 409, "{result}");
    assert_eq!(result["error"]["kind"], "record_ownership_conflict");
    assert_eq!(read(&host), before);
}

/// Implementation aid: an omitted owner must not bypass the Resolution owner check.
#[test]
fn omitted_resolution_owner_cannot_submit_or_withdraw() {
    let repo = repository_with_actors(&["reviewer"]);
    create_resolution(repo.path(), Some("shaping_agent"));
    let host = start(repo.path());
    let before = read(&host);
    let (status, result) = submit(&host, None);
    assert_eq!(status, 409, "{result}");
    assert_eq!(result["error"]["kind"], "record_ownership_conflict");
    assert_eq!(read(&host), before);
    let proposal = pending(&host);
    let before = read(&host);
    let (status, result) = post(
        &host,
        &format!("/resolutions/res_reject/submissions/{proposal}/withdraw"),
        &json!({"actor":"shaping_agent", "reason":"Revise the result."}),
    );
    assert_eq!(status, 409, "{result}");
    assert_eq!(result["error"]["kind"], "record_ownership_conflict");
    assert_eq!(read(&host), before);
}

/// Implementation aid: a manual Resolution still takes no declared owner.
#[test]
fn unowned_resolution_can_submit_and_withdraw_without_an_owner() {
    let repo = repository_with_actors(&["reviewer"]);
    create_resolution(repo.path(), None);
    let host = start(repo.path());
    let (status, result) = submit(&host, None);
    assert_eq!(status, 200, "{result}");
    let proposal = result["data"]["proposal_id"].as_str().unwrap();
    let (status, result) = post(
        &host,
        &format!("/resolutions/res_reject/submissions/{proposal}/withdraw"),
        &json!({"actor":"cli", "reason":"Revise the result."}),
    );
    assert_eq!(status, 200, "{result}");
    let after = read(&host);
    assert!(after["decision"]["pending"].is_null());
    assert_eq!(after["decision"]["withdrawn"], json!([proposal]));
}
