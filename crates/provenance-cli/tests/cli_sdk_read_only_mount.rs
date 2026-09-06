#![cfg(target_os = "linux")]

#[path = "query_support/fixtures.rs"]
mod fixtures;

use provenance_macros::verifies;
use serde_json::{json, Value};
use std::io::Write;
use std::process::{Command, Stdio};

#[test]
#[verifies("rule_read_only_checkout_answers_as_an_immutable_image", examples)]
fn a_read_only_mount_answers_without_the_publication_lock() {
    let available = Command::new("unshare")
        .args(["--user", "--map-root-user", "--mount", "true"])
        .output();
    if !available.is_ok_and(|output| output.status.success()) {
        return;
    }
    let repo = fixtures::init_repo();
    let path = repo.path().to_str().unwrap();
    let input = json!({"node_type": "requirement", "id": "req_missing"});
    let before = fixtures::sdk(path, "get", &input);
    let cache = repo.path().join(".provenance/cache");
    let binary = assert_cmd::cargo::cargo_bin!("provenance");
    for (policy, outcome) in [
        ("annotate_only", "annotate_only"),
        ("catch_up", "catch_up_failed"),
    ] {
        let mut child = Command::new("unshare")
            .args(["--user", "--map-root-user", "--mount", "sh", "-c",
                "mount --bind \"$1\" \"$1\" && mount -o remount,bind,ro \"$1\" && exec \"$2\" sdk get --repo \"$3\" --freshness \"$4\"",
                "read-only-mount"])
            .arg(&cache).arg(binary).arg(path).arg(policy)
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
            .spawn().unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.to_string().as_bytes())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{policy}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let answer: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(answer["stamp"]["serial"], before["stamp"]["serial"]);
        assert_eq!(answer["stamp"]["digest"], before["stamp"]["digest"]);
        assert_eq!(answer["stamp"]["policy"], outcome);
        assert_eq!(
            answer.get("freshness_error").is_some(),
            policy == "catch_up"
        );
        assert!(!cache.join("provenance.db-wal").exists());
        assert!(!cache.join("provenance.db-shm").exists());
    }
}
