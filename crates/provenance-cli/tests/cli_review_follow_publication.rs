#[path = "review_host_support/mod.rs"]
mod review_host_support;
use review_host_support::{
    follow::{create, Follow},
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
            scope.spawn(|| create(repo.path(), id, "rule", &["--requirement-id", "req_follow"]));
        }
    });
    let current = follow.until_all(&["rule_first", "rule_second"]);
    assert_eq!(current["type"], "state");
    assert_eq!(current["continuity"], "current");
}

#[test]
// Implementation aid: a writer that races initial connection cannot disappear from the stream.
fn a_write_during_subscription_start_reaches_the_document() {
    let repo = repository();
    create(repo.path(), "req_follow", "requirement", &[]);
    let host = start(repo.path());
    let follow = Follow::start(&host, repo.path(), "req_follow");
    create(
        repo.path(),
        "rule_racing",
        "rule",
        &["--requirement-id", "req_follow"],
    );
    let current = follow.until_all(&["req_follow", "rule_racing"]);
    assert_eq!(current["type"], "state");
    assert!(current["records"].as_array().unwrap().len() >= 2);
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
    let output = waits::output(host.cli().args([
        "requirements",
        "req_follow",
        "update",
        "--statement",
        "This write must fail.",
        "--if-match",
        &format!("sha256:{}", "0".repeat(64)),
        "--repo",
        repo.path().to_str().unwrap(),
    ]));
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr)
        .to_lowercase()
        .contains("precondition"));
    assert_eq!(std::fs::read(shard).unwrap(), before);
    follow.quiet();
}

#[test]
// Implementation aid: moving a member to another root must report its removal from this root.
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
    let output = waits::output(host.cli().args([
        "rules",
        "rule_follow",
        "update",
        "--requirement-id",
        "req_other",
        "--if-match",
        record["data"]["edit"]["etag"].as_str().unwrap(),
        "--repo",
        repo.path().to_str().unwrap(),
    ]));
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let current = follow.frame();
    assert_eq!(current["type"], "state");
    assert_eq!(current["removedIds"], serde_json::json!(["rule_follow"]));
    assert!(!current["records"].to_string().contains("rule_follow"));
}
