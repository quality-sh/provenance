#[path = "review_host_support/mod.rs"]
mod review_host_support;
use review_host_support::{
    follow::{create, page_request, page_rule, Follow},
    repository, start, waits,
};

#[test]
// Implementation aid: two public writers must both appear in the current document.
fn two_writers_reach_the_same_subscription() {
    let repo = repository();
    create(repo.path(), "req_follow", "requirement", &[]);
    let host = start(repo.path());
    let follow = Follow::start(&host, repo.path(), "req_follow");
    follow.frame();
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
    let repo = repository();
    create(repo.path(), "req_follow", "requirement", &[]);
    let host = start(repo.path());
    let follow = Follow::start(&host, repo.path(), "req_follow");
    follow.frame();
    let shard = repo
        .path()
        .join(".provenance/state/scopes/default/requirements/req.jsonl");
    let before = std::fs::read(&shard).unwrap();
    let response = review_host_support::response(
        page_request(&host, "PATCH", "/requirements/req_follow")
            .set("If-Match", &format!("\"sha256:{}\"", "0".repeat(64)))
            .send_string(
                &serde_json::json!({"data":{
                    "statement":"The system records the changed statement."
                }})
                .to_string(),
            ),
    );
    assert_eq!(response.status(), 409);
    let failure: serde_json::Value = serde_json::from_str(&response.into_string().unwrap()).unwrap();
    assert_eq!(failure["error"]["kind"], "requirement_edit_conflict");
    assert_eq!(std::fs::read(shard).unwrap(), before);
    follow.quiet();
}

#[test]
// Implementation aid: moving a member to another root must report its removal from this root.
#[provenance_macros::verifies("rule_review_follow_people_only", examples)]
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
