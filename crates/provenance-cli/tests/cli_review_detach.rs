//! A detached review host keeps serving after the command that started it returns, and a
//! second detached start reuses it instead of starting another.
//!
//! Process control uses `kill`, so these checks run on Unix.
#![cfg(unix)]

#[path = "review_host_support/mod.rs"]
mod review_host_support;

use provenance_macros::verifies;
use review_host_support::{repository, waits, BrowserRecorder};
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

/// Stops the background host when the test ends, whatever the outcome.
struct Detached(u32);

impl Drop for Detached {
    fn drop(&mut self) {
        let _ = Command::new("kill")
            .args(["-TERM", &self.0.to_string()])
            .status();
    }
}

/// The per-user cache that the host and the CLI share, as one isolated directory.
struct UserCache(tempfile::TempDir);

impl UserCache {
    fn new() -> Self {
        Self(tempfile::tempdir().unwrap())
    }

    fn apply(&self, command: &mut Command) {
        let caches = self.0.path().join("Library").join("Caches");
        command
            .env("HOME", self.0.path())
            .env("XDG_CACHE_HOME", &caches)
            .env("LOCALAPPDATA", &caches)
            .env(
                "PROVENANCE_TEST_STE100_ASSET_URL",
                "http://127.0.0.1:9/unavailable",
            );
    }
}

fn detach_command(root: &Path, cache: &UserCache) -> Command {
    let mut command = Command::new(assert_cmd::cargo::cargo_bin("provenance"));
    command.args([
        "review",
        "--repo",
        root.to_str().unwrap(),
        "--repository-id",
        "A",
        "--scope",
        "default",
        "--detach",
    ]);
    cache.apply(&mut command);
    command
}

fn detach(root: &Path, cache: &UserCache, configure: impl FnOnce(&mut Command)) -> Value {
    let mut command = detach_command(root, cache);
    configure(&mut command);
    let output = waits::output(&mut command);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn identity(endpoint: &str) -> Result<ureq::Response, ureq::Error> {
    ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(2))
        .build()
        .get(&format!("{endpoint}/local-host-identity"))
        .call()
}

fn wait_until(condition: impl Fn() -> bool, what: &str) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while !condition() {
        assert!(Instant::now() < deadline, "{what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn registry(root: &Path) -> PathBuf {
    root.join(".provenance/cache/local-hosts/default.json")
}

#[test]
/// The command returns while the host still answers, and the next start finds that host.
#[verifies("rule_review_detach_outlives_caller", examples)]
#[verifies("rule_review_detach_reuses_running_host", examples)]
fn a_detached_host_outlives_the_command_and_is_reused() {
    let repo = repository();
    let cache = UserCache::new();

    let started = detach(repo.path(), &cache, |command| {
        command.arg("--no-open");
    });
    let pid = u32::try_from(started["pid"].as_u64().expect("pid of the started host")).unwrap();
    let _stop = Detached(pid);
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

    // The parent process has returned; the host still answers from its own process.
    let answer: Value =
        serde_json::from_str(&identity(&endpoint).unwrap().into_string().unwrap()).unwrap();
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
        || identity(&endpoint).is_err(),
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
    let cache = UserCache::new();
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
/// With a browser available, the start opens the signed-in page and still prints the link.
#[verifies("rule_review_detach_reuses_running_host", examples)]
fn a_detached_start_opens_the_page_when_a_browser_is_available() {
    let repo = repository();
    let cache = UserCache::new();
    let browser = BrowserRecorder::install();

    let started = detach(repo.path(), &cache, |command| browser.configure(command));
    let _stop = Detached(u32::try_from(started["pid"].as_u64().unwrap()).unwrap());
    let endpoint = started["endpoint"].as_str().unwrap();

    assert_eq!(started["opened"], Value::Bool(true));
    assert_eq!(started["review_url"], Value::String(format!("{endpoint}/")));
    let opened = browser.wait_for_link(Duration::from_secs(10));
    assert!(opened.starts_with(&format!("{endpoint}/#launch=")), "{opened}");
}
