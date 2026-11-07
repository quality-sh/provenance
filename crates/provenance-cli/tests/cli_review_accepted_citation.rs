//! A Resolution that the reviewer approves on the review page can be cited at once. The
//! approval leaves the lifecycle status proposed, and the citation check reads the
//! accepted current revision as approval.
#[path = "review_host_support/mod.rs"]
mod review_host_support;

use provenance_macros::verifies;
use review_host_support::{
    repository_with_actors, request, resolution::create_resolution, response, start, Host,
};
use serde_json::{json, Value};
use std::path::Path;

fn send(host: &Host, method: &str, path: &str, etag: Option<&str>, data: &Value) -> (u16, Value) {
    let mut call = request(host, method, path, true).set("Content-Type", "application/json");
    if let Some(etag) = etag {
        call = call.set("If-Match", &format!("\"{etag}\""));
    }
    let result = response(call.send_string(&json!({ "data": data }).to_string()));
    let status = result.status();
    let body: Value = serde_json::from_str(&result.into_string().unwrap()).unwrap();
    println!("{method} {path} {data}\nHTTP {status} {body}");
    (status, body)
}

fn read(host: &Host, path: &str) -> Value {
    let result = request(host, "GET", path, true).call().unwrap();
    serde_json::from_str::<Value>(&result.into_string().unwrap()).unwrap()["data"].clone()
}

fn create(root: &Path, args: &[&str], data: &Value) {
    let mut command = std::process::Command::new(assert_cmd::cargo::cargo_bin("provenance"));
    command
        .args(args)
        .args([
            "--stdin",
            "--repo",
            root.to_str().unwrap(),
            "--format",
            "json",
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = command.spawn().unwrap();
    std::io::Write::write_all(
        &mut child.stdin.take().unwrap(),
        data.to_string().as_bytes(),
    )
    .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// A proposed Resolution, a Rule in review and an open Question that both cite it.
fn setup() -> (tempfile::TempDir, Host) {
    let repo = repository_with_actors(&["reviewer"]);
    create_resolution(repo.path(), None);
    create(
        repo.path(),
        &["rule_keep_result", "create", "--type", "rule"],
        &json!({
            "statement":"The system keeps each result.", "severity":"medium",
            "status":"review", "requirement_ids":["req_reject"], "resolution_ids":["res_reject"]
        }),
    );
    create(
        repo.path(),
        &["topic_result", "create", "--type", "topic"],
        &json!({"requirement_id":"req_reject", "title":"Result storage", "status":"open", "links":[]}),
    );
    create(
        repo.path(),
        &["question_result", "create", "--type", "question"],
        &json!({"topic_id":"topic_result", "question":"Does the system keep each result?",
               "resolution_method":"grill"}),
    );
    let host = start(repo.path());
    (repo, host)
}

/// Approves the current revision of the Resolution as the review page does.
fn approve(host: &Host) {
    let proposal = read(host, "/resolutions/res_reject")["decision"]["pending"]["proposal_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let (status, body) = send(
        host,
        "POST",
        &format!("/resolutions/res_reject/submissions/{proposal}/decide"),
        None,
        &json!({
            "actor":{"identity_type":"human","id":"reviewer"}, "decision":"accepted",
            "canonical_artifact":{"artifact_type":"resolution","artifact_id":"res_reject"}
        }),
    );
    assert_eq!(status, 200, "{body}");
    let after = read(host, "/resolutions/res_reject");
    // The approval does not change the lifecycle status.
    assert_eq!(after["status"], "proposed");
    assert!(
        !after["decision"]["current_acceptance"].is_null(),
        "{after}"
    );
}

#[test]
#[verifies("rule_review_acceptance_approves_resolution_citation", examples)]
fn an_answer_can_cite_a_resolution_that_the_reviewer_approved() {
    let (_repo, host) = setup();
    approve(&host);
    let (status, body) = answer(&host);
    assert_eq!(status, 200, "{body}");
    assert_eq!(
        read(&host, "/questions/question_result")["status"],
        "answered"
    );
}

#[test]
#[verifies("rule_review_acceptance_approves_resolution_citation", examples)]
fn a_rule_can_become_active_when_its_resolution_was_approved_in_review() {
    let (_repo, host) = setup();
    approve(&host);
    let rule = read(&host, "/rules/rule_keep_result");
    let (status, body) = send(
        &host,
        "PATCH",
        "/rules/rule_keep_result",
        rule["edit"]["etag"].as_str(),
        &json!({"status":"active"}),
    );
    assert_eq!(status, 200, "{body}");
    assert_eq!(read(&host, "/rules/rule_keep_result")["status"], "active");
}

/// Answers the Question with a citation of the Resolution.
fn answer(host: &Host) -> (u16, Value) {
    send(
        host,
        "POST",
        "/questions/question_result/answer",
        None,
        &json!({"answer":"Yes. The system keeps each result.", "resolution_id":"res_reject"}),
    )
}

#[test]
#[verifies("rule_review_acceptance_approves_resolution_citation", examples)]
fn a_resolution_without_an_accepted_current_revision_is_still_refused() {
    let (_repo, host) = setup();
    let (status, body) = answer(&host);
    assert_eq!(status, 400, "{body}");
    assert_eq!(body["error"]["kind"], "resolution_not_approved");
    approve(&host);
    // A content change makes a new revision that no reviewer accepted.
    let resolution = read(&host, "/resolutions/res_reject");
    let (status, body) = send(
        &host,
        "PATCH",
        "/resolutions/res_reject",
        resolution["edit"]["etag"].as_str(),
        &json!({"position":"The system stores each result for one year."}),
    );
    assert_eq!(status, 200, "{body}");
    let (status, body) = answer(&host);
    assert_eq!(status, 400, "{body}");
    assert_eq!(body["error"]["kind"], "resolution_not_approved");
    assert_eq!(read(&host, "/questions/question_result")["status"], "open");
}
