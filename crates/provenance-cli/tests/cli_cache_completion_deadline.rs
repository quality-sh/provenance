//! A read whose cache close cannot take the close lock fails at the
//! completion deadline and names the lock, instead of running until the
//! process is killed.

use fs2::FileExt;
use provenance_macros::verifies;
use serde_json::json;
use std::time::{Duration, Instant};

mod support {
    pub mod review_paths;
}
use support::review_paths::{init, provenance};

#[test]
#[verifies("rule_cache_completion_names_its_stage_at_the_deadline", examples)]
fn a_held_close_lock_fails_the_read_naming_the_lock() {
    let (directory, repo) = init();
    for (collection, body) in [
        (
            "requirements",
            json!({"id":"req_deadline", "statement":"The system stores records."}),
        ),
        (
            "topics",
            json!({"id":"topic_deadline", "requirement_id":"req_deadline", "title":"Deadline"}),
        ),
    ] {
        provenance()
            .args([collection, "create", "--repo", &repo, "--stdin"])
            .write_stdin(body.to_string())
            .assert()
            .success();
    }
    let path = directory
        .path()
        .join(".provenance/cache/provenance.db.close.lock");
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .unwrap();
    lock.lock_exclusive().unwrap();

    let mut command = provenance();
    command.args(["topics", "topic_deadline", "get", "--repo", &repo]);
    let started = Instant::now();
    let output = command.output().unwrap();
    let elapsed = started.elapsed();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        elapsed < Duration::from_secs(40),
        "the read ran for {elapsed:?}: {command:?}; status: {:?}; stderr: {stderr}",
        output.status
    );
    assert!(!output.status.success(), "the read succeeded: {stderr}");
    assert!(
        stderr.contains(&format!("waiting for the close lock {}", path.display())),
        "{stderr}"
    );
    FileExt::unlock(&lock).unwrap();
}
