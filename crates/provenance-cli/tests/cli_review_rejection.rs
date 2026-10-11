#[path = "review_host_support/mod.rs"]
mod review_host_support;

use provenance_macros::verifies;
use review_host_support::{repository_with_actors, request, response, start, Host};
use serde_json::{json, Value};
use std::{io::Write as _, path::Path};

fn create_resolution(root: &Path) {
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
                "status":"proposed", "made_by":"shaping_agent",
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

fn post(host: &Host, path: &str, data: Value) -> (u16, Value) {
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

fn pending(host: &Host) -> String {
    let (status, submitted) = post(
        host,
        "/resolutions/res_reject/submit",
        json!({
            "actor":"shaping_agent", "title":"Keep the result",
            "summary":"The system stores each result.", "source_ids":[],
            "evidence_references":[], "builds_on":[]
        }),
    );
    assert_eq!(status, 200, "{submitted}");
    read(host)["decision"]["pending"]["proposal_id"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn rejection() -> Value {
    // This is the body that the review page sends for a Resolution rejection.
    json!({
        "actor":{"identity_type":"human","id":"reviewer"},
        "decision":"rejected", "rationale":"The result needs a clear condition.",
        "feedback":{"body":"The result needs a clear condition.","role":"user"}
    })
}

#[test]
#[verifies("rule_review_rejection_requires_comment", examples)]
#[verifies("rule_rejection_keeps_graph_record", examples)]
fn page_rejection_stores_the_reason_and_keeps_the_resolution() {
    let repo = repository_with_actors(&["reviewer"]);
    create_resolution(repo.path());
    let host = start(repo.path());
    let proposal = pending(&host);
    let before = read(&host);
    let (status, result) = post(
        &host,
        &format!("/resolutions/res_reject/submissions/{proposal}/decide"),
        rejection(),
    );
    assert_eq!(status, 200, "{result}");

    let after = read(&host);
    assert!(after["decision"]["pending"].is_null());
    let decisions = after["decision"]["decisions"].as_array().unwrap();
    assert_eq!(decisions.len(), 1);
    assert_eq!(decisions[0]["disposition"]["proposal_id"], proposal);
    assert_eq!(decisions[0]["disposition"]["decision"], "rejected");
    assert_eq!(
        decisions[0]["disposition"]["rationale"],
        "The result needs a clear condition."
    );
    for field in ["id", "status", "position", "rationale", "made_by", "edit"] {
        assert_eq!(after[field], before[field], "{field}");
    }
    let discussions = request(&host, "GET", "/resolutions/res_reject/discussions", true)
        .call()
        .unwrap();
    let discussions: Value = serde_json::from_str(&discussions.into_string().unwrap()).unwrap();
    assert_eq!(discussions["data"]["items"].as_array().unwrap().len(), 1);
    let discussion = &discussions["data"]["items"][0];
    assert_eq!(discussion["actor"], "reviewer");
    assert_eq!(
        discussion["disposition_id"],
        result["data"]["disposition_id"]
    );
    let discussion_id = discussion["discussion_id"].as_str().unwrap();
    let messages = request(
        &host,
        "GET",
        &format!("/resolutions/res_reject/discussions/{discussion_id}/messages"),
        true,
    )
    .call()
    .unwrap();
    let messages: Value = serde_json::from_str(&messages.into_string().unwrap()).unwrap();
    assert_eq!(
        messages["data"]["items"][0]["body"],
        "The result needs a clear condition."
    );
}

#[test]
/// Implementation aid: an explicit wrong owner must leave the decision and feedback unwritten.
fn wrong_feedback_owner_leaves_the_submission_pending() {
    let repo = repository_with_actors(&["reviewer"]);
    create_resolution(repo.path());
    let host = start(repo.path());
    let proposal = pending(&host);
    let before = read(&host);
    let mut data = rejection();
    data["declared_by"] = json!("another_agent");
    let (status, result) = post(
        &host,
        &format!("/resolutions/res_reject/submissions/{proposal}/decide"),
        data,
    );
    assert_eq!(status, 409, "{result}");
    assert_eq!(result["error"]["kind"], "record_ownership_conflict");
    assert_eq!(read(&host), before);
    let discussions = request(&host, "GET", "/resolutions/res_reject/discussions", true)
        .call()
        .unwrap();
    let discussions: Value = serde_json::from_str(&discussions.into_string().unwrap()).unwrap();
    assert_eq!(discussions["data"]["items"], json!([]));
}
