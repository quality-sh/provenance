#[path = "review_host_support/mod.rs"]
mod review_host_support;

use review_host_support::{
    follow::{create, page_request, Follow},
    repository, start, waits,
};

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
    assert_eq!(page["continuations"], serde_json::json!([]));
    let seen = page["records"].to_string();
    assert!(seen.contains("req_follow") && seen.contains("rule_follow"));
    follow.quiet();
}
