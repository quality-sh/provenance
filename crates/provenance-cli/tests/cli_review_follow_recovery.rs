#[path = "review_host_support/mod.rs"]
mod review_host_support;
use provenance_store::layout::ProvenanceLayout;
use review_host_support::{
    follow::{create, setup, Follow},
    repository, start, waits,
};

#[test]
// Implementation aid: host loss must be explicit instead of a silent successful EOF.
fn host_loss_reports_unknown_continuity_and_fails() {
    let (_repo, host, mut follow, initial) = setup();
    assert_eq!(initial["type"], "state");
    drop(host);
    let loss = follow.frame();
    assert_eq!(loss["type"], "disconnected");
    assert_eq!(loss["continuity"], "unknown");
    let end = std::time::Instant::now() + std::time::Duration::from_secs(15);
    loop {
        if let Some(status) = follow.child.try_wait().unwrap() {
            assert!(!status.success());
            break;
        }
        assert!(std::time::Instant::now() < end);
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
}

#[cfg(unix)]
#[test]
// Implementation aid: user cancellation must stop a blocked subscription with success.
fn interrupt_stops_a_waiting_follow_command() {
    let (_repo, _host, mut follow, initial) = setup();
    assert_eq!(initial["type"], "state");
    let result = std::process::Command::new("kill")
        .args(["-INT", &follow.child.id().to_string()])
        .status()
        .unwrap();
    assert!(result.success());
    let end = std::time::Instant::now() + std::time::Duration::from_secs(15);
    loop {
        if let Some(status) = follow.child.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        assert!(std::time::Instant::now() < end);
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
}

#[test]
#[provenance_macros::verifies("rule_review_follow_reset", examples)]
fn configuration_reset_returns_current_document_with_unknown_continuity() {
    let (repo, host, follow, initial) = setup();
    assert_eq!(initial["type"], "state");
    let output = review_host_support::waits::output(host.cli().args([
        "init",
        "--path",
        repo.path().to_str().unwrap(),
        "--disposition-actor-id",
        "new-reviewer",
    ]));
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let reset = follow.frame();
    assert_eq!(reset["type"], "reset");
    assert_eq!(reset["continuity"], "unknown");
    assert!(reset["records"].to_string().contains("req_follow"));
}

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
    let (repo, host, follow, initial) = setup();
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
