//! A detached review host keeps serving after the command that started it returns, and a
//! second detached start reuses it instead of starting another.
//!
//! Process control uses `kill`, so these checks run on Unix.
#![cfg(unix)]

#[path = "review_host_support/mod.rs"]
mod review_host_support;

use provenance_macros::verifies;
use review_host_support::{
    detached::{
        detach, detach_command, identity, registry, started_pid, wait_until, Detached, UserCache,
    },
    repository, waits, BrowserRecorder,
};
use serde_json::Value;
use std::{path::Path, process::Command, time::Duration};

/// The host leads a new session, which has no controlling terminal, and reads nothing from
/// the caller. The checks read Linux process state, so other Unix runs check only that the
/// host answers.
#[cfg(target_os = "linux")]
fn assert_detached_from_caller(pid: u32) {
    let process = rustix::process::Pid::from_raw(i32::try_from(pid).unwrap()).unwrap();
    assert_eq!(rustix::process::getsid(Some(process)).unwrap(), process);
    assert_eq!(rustix::process::getpgid(Some(process)).unwrap(), process);
    assert_eq!(
        std::fs::read_link(format!("/proc/{pid}/fd/0")).unwrap(),
        Path::new("/dev/null")
    );
}

#[cfg(not(target_os = "linux"))]
const fn assert_detached_from_caller(_pid: u32) {}

#[test]
/// The command returns while the host still answers, and the next start finds that host.
#[verifies("rule_review_detach_outlives_caller", examples)]
#[verifies("rule_review_detach_reuses_running_host", examples)]
fn a_detached_host_outlives_the_command_and_is_reused() {
    let repo = repository();
    let cache = UserCache::isolated();

    let started = detach(repo.path(), &cache, |command| {
        command.arg("--no-open");
    });
    let pid = started_pid(&started);
    let _stop = Detached(pid);
    assert_detached_from_caller(pid);
    let endpoint = started["endpoint"].as_str().unwrap().to_owned();
    assert_eq!(started["reused"], Value::Bool(false));
    assert_eq!(started["opened"], Value::Bool(false));
    assert_eq!(started["url"], Value::String(format!("{endpoint}/")));
    let review_url = started["review_url"].as_str().unwrap();
    assert!(
        review_url.starts_with(&format!("{endpoint}/#launch=")),
        "{review_url}"
    );
    assert!(Path::new(started["log"].as_str().unwrap()).is_file());

    // The parent process has returned; the host still answers from its own session.
    let answer = identity(&endpoint).expect("the detached host answers");
    assert_eq!(answer["scope"], Value::String("default".to_owned()));
    assert!(registry(repo.path()).is_file());

    let reused = detach(repo.path(), &cache, |command| {
        command.arg("--no-open");
    });
    assert_eq!(reused["reused"], Value::Bool(true));
    assert_eq!(reused["endpoint"], Value::String(endpoint.clone()));
    assert!(reused.get("pid").is_none(), "{reused}");
    assert!(reused.get("log").is_none(), "{reused}");
    assert!(
        reused["review_url"]
            .as_str()
            .unwrap()
            .starts_with(&format!("{endpoint}/#launch=")),
        "{reused}"
    );

    // A record link uses the detached host and the key it published.
    let mut create = Command::new(assert_cmd::cargo::cargo_bin("provenance"));
    cache.apply(&mut create);
    create.args([
        "req_detached",
        "create",
        "--type",
        "requirement",
        "--repo",
        repo.path().to_str().unwrap(),
        "--statement",
        "The person follows the records on the review page.",
        "--format",
        "json",
    ]);
    let created = waits::output(&mut create);
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );
    let mut link = Command::new(assert_cmd::cargo::cargo_bin("provenance"));
    cache.apply(&mut link);
    link.args([
        "req_detached",
        "--review-link",
        "--no-open",
        "--repo",
        repo.path().to_str().unwrap(),
        "--format",
        "json",
    ]);
    let linked = waits::output(&mut link);
    assert!(
        linked.status.success(),
        "{}",
        String::from_utf8_lossy(&linked.stderr)
    );
    let linked: Value = serde_json::from_slice(&linked.stdout).unwrap();
    assert!(
        linked["review_url"]
            .as_str()
            .unwrap()
            .starts_with(&format!("{endpoint}/?root=req_detached#launch=")),
        "{linked}"
    );

    // SIGTERM stops the host, which removes its registration.
    assert!(Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .status()
        .unwrap()
        .success());
    wait_until(
        || identity(&endpoint).is_none(),
        "the host did not stop after SIGTERM",
    );
    wait_until(
        || !registry(repo.path()).exists(),
        "the stopped host left its registration",
    );
}

#[test]
/// A host that cannot start is reported with its own error, not as a timeout.
#[verifies("rule_review_detach_outlives_caller", examples)]
fn a_detached_host_that_cannot_bind_reports_the_cause() {
    let repo = repository();
    let cache = UserCache::isolated();
    let taken = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = taken.local_addr().unwrap().port().to_string();

    let mut command = detach_command(repo.path(), &cache);
    command.args(["--no-open", "--port", &port]);
    let output = waits::output(&mut command);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("before it published its location"),
        "{stderr}"
    );
    assert!(stderr.contains("cannot bind review listener"), "{stderr}");
    assert!(!registry(repo.path()).exists());
}

#[test]
/// With a browser available, a start and a reuse each open the signed-in page and print
/// the plain page address.
#[verifies("rule_review_detach_reuses_running_host", examples)]
fn a_detached_start_and_reuse_open_the_page_when_a_browser_is_available() {
    let repo = repository();
    let cache = UserCache::isolated();
    let browser = BrowserRecorder::install();

    let started = detach(repo.path(), &cache, |command| browser.configure(command));
    let _stop = Detached(started_pid(&started));
    let endpoint = started["endpoint"].as_str().unwrap();
    assert_eq!(started["opened"], Value::Bool(true));
    assert_eq!(started["review_url"], Value::String(format!("{endpoint}/")));
    let opened = browser.wait_for_link(Duration::from_secs(10));
    assert!(
        opened.starts_with(&format!("{endpoint}/#launch=")),
        "{opened}"
    );

    let second_browser = BrowserRecorder::install();
    let reused = detach(repo.path(), &cache, |command| {
        second_browser.configure(command)
    });
    assert_eq!(reused["reused"], Value::Bool(true));
    assert_eq!(reused["opened"], Value::Bool(true));
    assert_eq!(reused["review_url"], Value::String(format!("{endpoint}/")));
    let reopened = second_browser.wait_for_link(Duration::from_secs(10));
    assert!(
        reopened.starts_with(&format!("{endpoint}/#launch=")) && reopened != opened,
        "{reopened}"
    );
}

#[test]
/// A record left by a host that was killed is not reused; the next start replaces it.
#[verifies("rule_review_detach_reuses_running_host", examples)]
fn a_start_after_a_crash_replaces_the_dead_host() {
    let repo = repository();
    let cache = UserCache::isolated();
    let first = detach(repo.path(), &cache, |command| {
        command.arg("--no-open");
    });
    let first_pid = started_pid(&first);
    let first_endpoint = first["endpoint"].as_str().unwrap().to_owned();
    assert!(Command::new("kill")
        .args(["-KILL", &first_pid.to_string()])
        .status()
        .unwrap()
        .success());
    wait_until(
        || identity(&first_endpoint).is_none(),
        "the killed host still answers",
    );
    assert!(registry(repo.path()).is_file(), "SIGKILL leaves the record");

    let second = detach(repo.path(), &cache, |command| {
        command.arg("--no-open");
    });
    let second_pid = started_pid(&second);
    let _stop = Detached(second_pid);
    assert_eq!(second["reused"], Value::Bool(false));
    // One log per repository and scope: the new host replaces the log of the dead one.
    assert_eq!(second["log"], first["log"]);
    assert_ne!(second_pid, first_pid);
    let endpoint = second["endpoint"].as_str().unwrap();
    assert!(identity(endpoint).is_some(), "the new host answers");
    assert!(
        second["review_url"]
            .as_str()
            .unwrap()
            .starts_with(&format!("{endpoint}/#launch=")),
        "{second}"
    );
}

#[test]
/// Concurrent starts for one repository start one host; another repository gets its own.
#[verifies("rule_review_detach_reuses_running_host", examples)]
fn concurrent_starts_share_one_host_per_repository() {
    let first = repository();
    let second = repository();
    let cache = UserCache::isolated();
    let roots = [first.path(), first.path(), first.path(), second.path()];
    // Spawn every start before waiting for any, so the starts overlap.
    let mut children = Vec::with_capacity(roots.len());
    for root in roots {
        let mut command = detach_command(root, &cache);
        command
            .arg("--no-open")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        children.push(command.spawn().unwrap());
    }
    // Collect every output and its stop guard before any assertion, so a failed start
    // leaves no host running.
    let finished: Vec<_> = children
        .into_iter()
        .map(|child| child.wait_with_output().unwrap())
        .collect();
    let outputs: Vec<Value> = finished
        .iter()
        .map(|output| serde_json::from_slice(&output.stdout).unwrap_or(Value::Null))
        .collect();
    let _stops: Vec<_> = outputs
        .iter()
        .filter_map(|output| output["pid"].as_u64())
        .map(|pid| Detached(u32::try_from(pid).unwrap()))
        .collect();
    for output in &finished {
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let (shared, other) = outputs.split_at(3);
    let started = shared
        .iter()
        .filter(|output| output["reused"] == Value::Bool(false))
        .count();
    assert_eq!(started, 1, "{shared:?}");
    assert!(
        shared
            .iter()
            .all(|output| output["endpoint"] == shared[0]["endpoint"]),
        "{shared:?}"
    );
    assert_eq!(other[0]["reused"], Value::Bool(false));
    assert_ne!(other[0]["endpoint"], shared[0]["endpoint"]);
    for output in &outputs {
        assert!(identity(output["endpoint"].as_str().unwrap()).is_some());
    }
}
