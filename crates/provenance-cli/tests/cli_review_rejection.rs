#[path = "review_host_support/mod.rs"]
mod review_host_support;

use provenance_macros::verifies;
use review_host_support::{
    repository_with_actors, request, resolution::create_resolution, response, start, Host,
};
use serde_json::{json, Value};

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

fn pending(host: &Host) -> String {
    let (status, submitted) = post(
        host,
        "/resolutions/res_reject/submit",
        &json!({
            "actor":"shaping_agent", "declared_by":"shaping_agent", "title":"Keep the result",
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
    create_resolution(repo.path(), Some("shaping_agent"));
    let host = start(repo.path());
    let proposal = pending(&host);
    let before = read(&host);
    let (status, result) = post(
        &host,
        &format!("/resolutions/res_reject/submissions/{proposal}/decide"),
        &rejection(),
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
    let discussion_id = discussions["data"]["items"][0]["discussion"]["discussion_id"]
        .as_str()
        .unwrap();
    let discussion = request(
        &host,
        "GET",
        &format!("/resolutions/res_reject/discussions/{discussion_id}"),
        true,
    )
    .call()
    .unwrap();
    let discussion: Value = serde_json::from_str(&discussion.into_string().unwrap()).unwrap();
    let discussion = &discussion["data"]["discussion"];
    assert_eq!(discussion["actor"], "reviewer");
    assert_eq!(
        discussion["disposition_id"],
        result["data"]["disposition_id"]
    );
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
    create_resolution(repo.path(), Some("shaping_agent"));
    let host = start(repo.path());
    let proposal = pending(&host);
    let before = read(&host);
    let mut data = rejection();
    data["declared_by"] = json!("another_agent");
    let (status, result) = post(
        &host,
        &format!("/resolutions/res_reject/submissions/{proposal}/decide"),
        &data,
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

fn document(host: &Host) -> Value {
    let result = request(
        host,
        "GET",
        "/requirements/req_reject/document?exclude_terminal=true",
        true,
    )
    .call()
    .unwrap();
    serde_json::from_str::<Value>(&result.into_string().unwrap()).unwrap()["data"].clone()
}

fn pending_ids(document: &Value) -> Vec<String> {
    document["entries"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| entry["review"]["outcome"] == "pending")
        .map(|entry| entry["node"]["id"].as_str().unwrap().to_owned())
        .collect()
}

fn ids(document: &Value) -> Vec<&str> {
    document["entries"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|entry| entry["node"]["id"].as_str())
        .collect()
}

#[test]
#[verifies("rule_review_defaults_exclude_terminal_records", examples)]
fn cli_rejection_removes_the_resolution_from_the_review_document() {
    let repo = repository_with_actors(&["reviewer"]);
    create_resolution(repo.path());
    let host = start(repo.path());
    pending(&host);
    let before = document(&host);
    assert!(pending_ids(&before).contains(&"res_reject".to_owned()));
    let pending_before = before["review_totals"]["pending"].as_u64().unwrap();

    // The agent rejects the proposal through the CLI after the owner decides.
    let output = std::process::Command::new(assert_cmd::cargo::cargo_bin("provenance"))
        .args(["resolutions", "res_reject", "update", "--repo"])
        .arg(repo.path())
        .args(["--status", "rejected"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let after = document(&host);
    assert!(!ids(&after).contains(&"res_reject"), "{after}");
    assert!(!pending_ids(&after).contains(&"res_reject".to_owned()));
    assert_eq!(
        after["review_totals"]["pending"].as_u64().unwrap(),
        pending_before - 1
    );
    assert_eq!(read(&host)["status"], "rejected");
}
