#[path = "review_host_support/mod.rs"]
mod review_host_support;
use review_host_support::{
    follow::{create, page_request, page_rule, pending_proposal, setup, Follow},
    repository, repository_with_actors, request, response, start, waits,
};
use serde_json::json;

#[test]
#[provenance_macros::verifies("rule_review_follow_push", examples)]
#[provenance_macros::verifies("rule_review_follow_people_only", examples)]
fn follow_receives_only_the_question_written_by_the_review_page() {
    let repo = repository();
    create(repo.path(), "req_follow", "requirement", &[]);
    let output = waits::output(
        std::process::Command::new(assert_cmd::cargo::cargo_bin("provenance")).args([
            "topic_follow",
            "create",
            "--type",
            "topic",
            "--requirement-id",
            "req_follow",
            "--title",
            "Review delivery",
            "--repo",
            repo.path().to_str().unwrap(),
        ]),
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let host = start(repo.path());
    let follow = Follow::start(&host, repo.path(), "req_follow");
    assert_eq!(follow.frame()["type"], "state");
    let saved = review_host_support::response(
        page_request(&host, "POST", "/questions").send_string(
            &serde_json::json!({"data":{
                "id":"q_follow", "topic_id":"topic_follow",
                "question":"Which changes reach the agent?", "resolution_method":"research",
                "status":"open", "links":[]
            }})
            .to_string(),
        ),
    );
    assert_eq!(saved.status(), 200, "{}", saved.into_string().unwrap());
    let changed = follow.until("q_follow");
    assert_eq!(changed["type"], "state");
    assert_eq!(changed["continuity"], "current");
    assert!(changed["sequence"].as_u64().unwrap() > 0);
    assert_eq!(changed["records"].as_array().unwrap().len(), 1);
    follow.quiet();
}

#[test]
// Implementation aid: current-state pages must remain bounded and finish before waiting.
fn follow_collects_bounded_reads_into_one_initial_document() {
    let repo = repository();
    create(repo.path(), "req_follow", "requirement", &[]);
    create(
        repo.path(),
        "rule_follow",
        "rule",
        &["--requirement-id", "req_follow"],
    );
    let host = start(repo.path());
    let follow = Follow::with_options(&host, repo.path(), "req_follow", &["--limit", "1"]);
    let page = follow.frame();
    assert_eq!(page["type"], "state");
    let seen = page["records"].to_string();
    assert!(seen.contains("req_follow") && seen.contains("rule_follow"));
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
    let follow = Follow::start(&host, repo.path(), "req_follow");
    let initial = follow.frame();
    assert_eq!(initial["records"].as_array().unwrap().len(), 2);
    let proposal = pending_proposal(&host, "req_follow");
    let saved = response(
        page_request(
            &host,
            "POST",
            &format!("/requirements/req_follow/submissions/{proposal}/decide"),
        )
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
    let (_repo, host, follow, _initial) = setup();
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
    let (_repo, host, follow, _initial) = setup();
    let proposal = pending_proposal(&host, "req_follow");
    let saved = response(
        page_request(
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

#[test]
// Implementation aid: two public writers must both appear in the current document.
fn two_writers_reach_the_same_subscription() {
    let (_repo, host, follow, _initial) = setup();
    std::thread::scope(|scope| {
        for id in ["rule_first", "rule_second"] {
            let host = &host;
            scope.spawn(move || page_rule(host, id, "req_follow"));
        }
    });
    let mut seen = String::new();
    while !seen.contains("rule_first") || !seen.contains("rule_second") {
        let current = follow.frame();
        assert_eq!(current["continuity"], "current");
        seen.push_str(&current.to_string());
    }
}

#[test]
// Implementation aid: a writer that races initial connection cannot disappear from the stream.
fn a_write_during_subscription_start_reaches_the_document() {
    let repo = repository();
    create(repo.path(), "req_follow", "requirement", &[]);
    let host = start(repo.path());
    let follow = Follow::start(&host, repo.path(), "req_follow");
    page_rule(&host, "rule_racing", "req_follow");
    let mut seen = String::new();
    while !seen.contains("req_follow") || !seen.contains("rule_racing") {
        let current = follow.frame();
        assert_eq!(current["type"], "state");
        seen.push_str(&current["records"].to_string());
    }
}

#[test]
// Implementation aid: a failed write precondition must not publish a state frame.
fn failed_publication_leaves_the_saved_document_and_stream_unchanged() {
    let (repo, host, follow, _initial) = setup();
    let shard = repo
        .path()
        .join(".provenance/state/scopes/default/requirements/req.jsonl");
    let before = std::fs::read(&shard).unwrap();
    let response = review_host_support::response(
        page_request(&host, "PATCH", "/requirements/req_follow")
            .set("If-Match", &format!("\"sha256:{}\"", "0".repeat(64)))
            .send_string(
                &serde_json::json!({"data":{
                    "actor":"agent", "statement":"The system records the changed statement."
                }})
                .to_string(),
            ),
    );
    let status = response.status();
    let body = response.into_string().unwrap();
    assert_eq!(status, 409, "{body}");
    let failure: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(failure["error"]["kind"], "requirement_edit_conflict");
    assert_eq!(std::fs::read(shard).unwrap(), before);
    follow.quiet();
}

#[test]
// Moving a member to another root must report its removal from this root.
#[provenance_macros::verifies("rule_review_follow_push", examples)]
fn changed_root_membership_removes_the_record_from_the_bound_document() {
    let repo = repository();
    create(repo.path(), "req_follow", "requirement", &[]);
    create(repo.path(), "req_other", "requirement", &[]);
    create(
        repo.path(),
        "rule_follow",
        "rule",
        &["--requirement-id", "req_follow"],
    );
    let host = start(repo.path());
    let follow = Follow::start(&host, repo.path(), "req_follow");
    assert!(follow.frame()["records"]
        .to_string()
        .contains("rule_follow"));
    let output = waits::output(host.cli().args([
        "rules",
        "rule_follow",
        "get",
        "--format",
        "json",
        "--repo",
        repo.path().to_str().unwrap(),
    ]));
    assert!(output.status.success());
    let record: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let saved = review_host_support::response(
        page_request(&host, "PATCH", "/rules/rule_follow")
            .set(
                "If-Match",
                &format!("\"{}\"", record["data"]["edit"]["etag"].as_str().unwrap()),
            )
            .send_string(
                &serde_json::json!({"data":{"requirement_ids":["req_other"]}}).to_string(),
            ),
    );
    assert_eq!(saved.status(), 200, "{}", saved.into_string().unwrap());
    let current = follow.frame();
    assert_eq!(current["type"], "state");
    assert_eq!(current["removedIds"], serde_json::json!(["rule_follow"]));
    assert!(!current["records"].to_string().contains("rule_follow"));
}
