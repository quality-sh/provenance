#[path = "review_host_support/mod.rs"]
mod review_host_support;
use review_host_support::{
    events::cookie,
    follow::{create, Follow},
    repository_with_actors, request, response, start,
};
use serde_json::{json, Value};

#[test]
#[provenance_macros::verifies("rule_review_follow_people_only", examples)]
fn agent_cli_write_does_not_notify_follow() {
    let repo = repository_with_actors(&["reviewer"]);
    create(repo.path(), "req_follow", "requirement", &[]);
    let host = start(repo.path());
    let follow = Follow::start(&host, repo.path(), "req_follow");
    follow.frame();
    create(
        repo.path(),
        "rule_agent",
        "rule",
        &["--requirement-id", "req_follow"],
    );
    follow.quiet();
}

#[test]
#[provenance_macros::verifies("rule_review_follow_people_only", examples)]
fn review_page_decision_sends_only_the_changed_record_once() {
    let repo = repository_with_actors(&["reviewer"]);
    create(repo.path(), "req_follow", "requirement", &[]);
    create(
        repo.path(),
        "rule_unchanged",
        "rule",
        &["--requirement-id", "req_follow"],
    );
    let host = start(repo.path());
    let session = cookie(&host);
    let follow = Follow::start(&host, repo.path(), "req_follow");
    let initial = follow.frame();
    assert_eq!(initial["records"].as_array().unwrap().len(), 2);
    let root: Value = serde_json::from_str(
        &response(request(&host, "GET", "/requirements/req_follow", true).call())
            .into_string()
            .unwrap(),
    )
    .unwrap();
    let proposal = root["data"]["decision"]["pending"]["proposal_id"]
        .as_str()
        .unwrap();
    let saved = response(
        request(
            &host,
            "POST",
            &format!("/requirements/req_follow/submissions/{proposal}/decide"),
            false,
        )
        .set("Cookie", &session)
        .set("Origin", host.config["endpoint"].as_str().unwrap())
        .set("Content-Type", "application/json")
        .send_string(
            &json!({"data":{
                "actor":{"identity_type":"human","id":"reviewer"},
                "decision":"accepted", "rationale":null,
                "canonical_artifact":{"artifact_type":"requirement","artifact_id":"req_follow"},
                "feedback":null, "declared_by":null
            }})
            .to_string(),
        ),
    );
    assert_eq!(saved.status(), 200, "{}", saved.into_string().unwrap());
    let changed = follow.frame();
    assert_eq!(changed["records"].as_array().unwrap().len(), 1);
    assert!(changed["records"].to_string().contains("accepted"));
    assert!(!changed["records"].to_string().contains("rule_unchanged"));
    assert_eq!(changed["continuations"], json!([]));
    follow.quiet();
    create(
        repo.path(),
        "rule_agent",
        "rule",
        &["--requirement-id", "req_follow"],
    );
    follow.quiet();
    review_host_support::follow::page_rule(&host, "rule_person", "req_follow");
    let next = follow.until("rule_person");
    assert_eq!(next["records"].as_array().unwrap().len(), 1);
    assert!(!next["records"].to_string().contains("rule_agent"));
    follow.quiet();
}

#[test]
#[provenance_macros::verifies("rule_review_follow_people_only", examples)]
fn bearer_sdk_write_does_not_notify_follow() {
    let repo = repository_with_actors(&["reviewer"]);
    create(repo.path(), "req_follow", "requirement", &[]);
    let host = start(repo.path());
    let follow = Follow::start(&host, repo.path(), "req_follow");
    follow.frame();
    let saved = response(
        request(&host, "POST", "/rules", true).send_string(
            &json!({"data":{
                "id":"rule_sdk", "statement":"The agent receives saved changes.",
                "status":"draft", "severity":"high", "requirement_ids":["req_follow"],
                "resolution_ids":[]
            }})
            .to_string(),
        ),
    );
    assert_eq!(saved.status(), 200, "{}", saved.into_string().unwrap());
    follow.quiet();
}

#[test]
#[provenance_macros::verifies("rule_review_follow_people_only", examples)]
fn review_page_rejection_without_a_canonical_artifact_reaches_the_agent() {
    let repo = repository_with_actors(&["reviewer"]);
    create(repo.path(), "req_follow", "requirement", &[]);
    let host = start(repo.path());
    let follow = Follow::start(&host, repo.path(), "req_follow");
    follow.frame();
    let root: Value = serde_json::from_str(
        &response(request(&host, "GET", "/requirements/req_follow", true).call())
            .into_string()
            .unwrap(),
    )
    .unwrap();
    let proposal = root["data"]["decision"]["pending"]["proposal_id"]
        .as_str()
        .unwrap();
    let saved = response(
        review_host_support::follow::page_request(
            &host,
            "POST",
            &format!("/requirements/req_follow/submissions/{proposal}/decide"),
        )
        .send_string(
            &json!({"data":{
                "actor":{"identity_type":"human","id":"reviewer"},
                "decision":"rejected", "rationale":"The requirement needs a change.",
                "canonical_artifact":null, "feedback":null, "declared_by":null
            }})
            .to_string(),
        ),
    );
    assert_eq!(saved.status(), 200, "{}", saved.into_string().unwrap());
    let changed = follow.until("rejected");
    assert!(changed["records"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["node"]["id"] == "req_follow"));
    follow.quiet();
}
