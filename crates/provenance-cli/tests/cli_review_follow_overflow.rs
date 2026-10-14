#[path = "review_host_support/mod.rs"]
mod review_host_support;
use review_host_support::{
    follow::{create, Follow},
    repository, start,
};

#[cfg(target_os = "linux")]
#[test]
#[provenance_macros::verifies("rule_review_follow_reset", examples)]
fn kernel_queue_overflow_reconciles_the_current_document() {
    use provenance_store::layout::ProvenanceLayout;
    let repo = repository();
    let replacement = repository();
    for root in [repo.path(), replacement.path()] {
        create(root, "req_follow", "requirement", &[]);
    }
    create(
        repo.path(),
        "rule_before",
        "rule",
        &["--requirement-id", "req_follow"],
    );
    create(
        replacement.path(),
        "rule_after",
        "rule",
        &["--requirement-id", "req_follow"],
    );
    let shard = ".provenance/state/scopes/default/rules/rule.jsonl";
    let bytes = std::fs::read(replacement.path().join(shard)).unwrap();
    let host = start(repo.path());
    let follow = Follow::start(&host, repo.path(), "req_follow");
    let initial = follow.frame();
    assert!(initial["records"].to_string().contains("rule_before"));
    let count: usize = std::fs::read_to_string("/proc/sys/fs/inotify/max_queued_events")
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert!(
        count <= 262_144,
        "overflow needs a dedicated runner for this queue size"
    );
    let layout = ProvenanceLayout::new(
        camino::Utf8PathBuf::from_path_buf(repo.path().to_path_buf()).unwrap(),
    );
    review_host_support::waits::publish(&layout, || {
        host.signal("-STOP");
        let result = (|| {
            for index in 0..count + 1024 {
                let path = repo
                    .path()
                    .join(format!(".provenance/state/event-{index}.noise"));
                std::fs::write(&path, "event")?;
                std::fs::remove_file(path)?;
            }
            let target = repo.path().join(shard);
            let temporary = target.with_extension("replacement");
            std::fs::write(&temporary, &bytes)?;
            std::fs::rename(temporary, target)
        })();
        host.signal("-CONT");
        Ok(result?)
    })
    .unwrap();
    let deadline = std::time::Instant::now() + review_host_support::waits::LIMIT;
    let mut removed = Vec::new();
    let reset = loop {
        assert!(
            std::time::Instant::now() < deadline,
            "overflow must deliver a reset"
        );
        let frame = follow.frame();
        removed.extend(frame["removedIds"].as_array().unwrap().iter().cloned());
        if frame["type"] == "reset" {
            break frame;
        }
        assert_eq!(frame["type"], "state");
    };
    assert_eq!(reset["type"], "reset");
    assert_eq!(reset["continuity"], "unknown");
    assert_eq!(reset["epoch"], initial["epoch"]);
    assert!(reset["records"].to_string().contains("rule_after"));
    assert_eq!(removed, vec![serde_json::json!("rule_before")]);
}

#[test]
#[provenance_macros::verifies("rule_review_follow_reset", examples)]
fn restart_requires_a_new_subscription_with_unknown_continuity() {
    let repo = repository();
    create(repo.path(), "req_follow", "requirement", &[]);
    let host = start(repo.path());
    let follow = Follow::start(&host, repo.path(), "req_follow");
    let initial = follow.frame();
    drop(host);
    assert_eq!(follow.frame()["type"], "disconnected");
    drop(follow);
    create(
        repo.path(),
        "rule_after",
        "rule",
        &["--requirement-id", "req_follow"],
    );
    let restarted = start(repo.path());
    let follow = Follow::start(&restarted, repo.path(), "req_follow");
    let current = follow.frame();
    assert_eq!(current["type"], "state");
    assert_eq!(current["continuity"], "unknown");
    assert_ne!(current["epoch"], initial["epoch"]);
    assert!(current["records"].to_string().contains("rule_after"));
}
