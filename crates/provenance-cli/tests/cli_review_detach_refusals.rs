//! A detached start that fails leaves no host running, and a reuse that asks for another
//! repository ID or port than the running host is refused.
//!
//! Process control uses `kill`, so these checks run on Unix.
#![cfg(unix)]

#[path = "review_host_support/mod.rs"]
mod review_host_support;

use provenance_macros::verifies;
use review_host_support::{
    detached::{detach, detach_command, registry, started_pid, Detached, UserCache},
    repository, waits,
};
use serde_json::Value;
use std::{
    os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _},
    time::Duration,
};

#[test]
/// A host that does not publish its location within the start limit is stopped.
#[verifies("rule_review_detach_outlives_caller", examples)]
fn a_host_that_does_not_publish_in_time_is_stopped() {
    let repo = repository();
    let cache = UserCache::isolated();
    // The test holds the publication lock of the slot, so the host cannot publish.
    let directory = repo.path().join(".provenance/cache/local-hosts");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700)).unwrap();
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .mode(0o600)
        .open(directory.join("default.lock"))
        .unwrap();
    fs2::FileExt::lock_exclusive(&lock).unwrap();

    let mut command = detach_command(repo.path(), &cache);
    command.arg("--no-open");
    let output = waits::output(&mut command);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("did not publish its location within 20 seconds, so it was stopped"),
        "{stderr}"
    );
    // A host that still ran would publish as soon as the lock is free.
    drop(lock);
    std::thread::sleep(Duration::from_secs(3));
    assert!(
        !registry(repo.path()).exists(),
        "the host still runs and published its location"
    );
}

#[test]
/// A reuse with another repository ID or port names the values of the running host.
#[verifies("rule_review_detach_reuses_running_host", examples)]
fn a_reuse_with_other_host_values_is_refused() {
    let repo = repository();
    let cache = UserCache::isolated();
    let started = detach(repo.path(), &cache, |command| {
        command.arg("--no-open");
    });
    let _stop = Detached(started_pid(&started));
    let endpoint = started["endpoint"].as_str().unwrap();
    let port = url::Url::parse(endpoint).unwrap().port().unwrap();
    let other_port = if port == u16::MAX { 1 } else { port + 1 };
    let named = format!("--repository-id A on --port {port}.");

    for (repository_id, port) in [("B", 0), ("A", other_port)] {
        let mut command = std::process::Command::new(assert_cmd::cargo::cargo_bin("provenance"));
        command.args(["review", "--repo", repo.path().to_str().unwrap()]);
        command.args(["--repository-id", repository_id, "--scope", "default"]);
        command.args(["--port", &port.to_string(), "--detach", "--no-open"]);
        cache.apply(&mut command);
        let output = waits::output(&mut command);
        assert!(!output.status.success(), "{repository_id} {port}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(&named), "{stderr}");
        assert!(output.stdout.is_empty(), "{repository_id} {port}");
    }

    let same = detach(repo.path(), &cache, |command| {
        command.args(["--no-open", "--port", &port.to_string()]);
    });
    assert_eq!(same["reused"], Value::Bool(true));
    assert_eq!(same["endpoint"], Value::String(endpoint.to_owned()));
}
