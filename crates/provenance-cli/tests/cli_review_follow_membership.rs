#[path = "review_host_support/mod.rs"]
mod review_host_support;
use provenance_store::layout::ProvenanceLayout;
use review_host_support::{
    follow::{create, Follow},
    repository, start, waits,
};

fn remove_shard(root: &std::path::Path, shard: &str) {
    let layout =
        ProvenanceLayout::new(camino::Utf8PathBuf::from_path_buf(root.to_path_buf()).unwrap());
    waits::publish(&layout, || {
        let target = root.join(format!(".provenance/state/scopes/default/{shard}"));
        assert!(
            target.exists(),
            "canonical shard must exist: {}",
            target.display()
        );
        let replacement = target.with_extension("replacement");
        std::fs::write(&replacement, "")?;
        std::fs::rename(replacement, target)?;
        Ok(())
    })
    .unwrap();
}

#[test]
// Implementation aid: writes outside the page do not produce agent messages.
fn external_removal_does_not_notify_the_agent() {
    let repo = repository();
    create(repo.path(), "req_follow", "requirement", &[]);
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
    remove_shard(repo.path(), "rules/rule.jsonl");
    follow.quiet();
}

#[test]
// Implementation aid: deleting the root is a read failure while the host remains available.
fn deleted_root_reports_a_read_error_when_recovery_reads_it() {
    let repo = repository();
    create(repo.path(), "req_follow", "requirement", &[]);
    let feed = review_host_support::scripted_feed::Feed::start(repo.path());
    feed.send(serde_json::json!({"type":"ready", "epoch":feed.epoch, "sequence":0}));
    let follow = feed.follow(repo.path());
    follow.frame();
    remove_shard(repo.path(), "requirements/req.jsonl");
    follow.quiet();
    feed.send(
        serde_json::json!({"type":"reset", "epoch":feed.epoch, "sequence":1, "reason":"recovery"}),
    );
    let error = follow.frame();
    assert_eq!(error["type"], "read-error");
    assert_eq!(error["continuity"], "unknown");
    assert_eq!(error["reason"], "document root is missing");
}
