use provenance_macros::verifies;
use std::{
    path::Path,
    process::{Child, Stdio},
    thread,
    time::{Duration, Instant},
};

#[path = "cli_dictionary/support.rs"]
#[allow(dead_code)]
mod dictionary_support;

#[path = "cli_ste_onboarding/support.rs"]
#[allow(dead_code)]
mod support;

use support::{init_args, serial, TestServer};

#[test]
#[verifies("rule_ste_dictionary_download_concurrency", examples)]
fn a_failing_download_does_not_block_a_concurrent_init() {
    let _serial = serial();
    let failing = TestServer::held(503, b"unavailable");
    let working = TestServer::new(200, dictionary_support::dictionary_pdf());
    let temporary = tempfile::tempdir().unwrap();
    let assets = temporary.path().join("assets");
    let indexes = temporary.path().join("indexes");
    let slow_repo = temporary.path().join("slow");
    let fast_repo = temporary.path().join("fast");

    let mut slow = spawn_init(&slow_repo, &assets, &indexes, &failing.url());
    wait_until("the first init requests the asset", || {
        !failing.requests().is_empty()
    });
    let mut fast = spawn_init(&fast_repo, &assets, &indexes, &working.url());
    wait_until("the second init stops", || {
        fast.try_wait().unwrap().is_some()
    });
    let slow_was_running = slow.try_wait().unwrap().is_none();
    failing.release();
    let fast = fast.wait_with_output().unwrap();
    let slow = slow.wait_with_output().unwrap();

    assert!(
        slow_was_running,
        "the second init waited for the failing download of the first init"
    );
    assert!(
        fast.status.success(),
        "{}",
        String::from_utf8_lossy(&fast.stderr)
    );
    assert!(dictionary_support::reference_path(&fast_repo).is_file());
    assert_eq!(working.requests().len(), 1);
    let slow_stderr = String::from_utf8_lossy(&slow.stderr);
    assert!(slow.status.success(), "{slow_stderr}");
    assert!(!slow_stderr.contains("Warning:"), "stderr: {slow_stderr}");
    assert!(
        dictionary_support::reference_path(&slow_repo).is_file(),
        "the first init uses the dictionary that the second init put in the cache"
    );
}

fn spawn_init(repo: &Path, assets: &Path, indexes: &Path, url: &str) -> Child {
    std::process::Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
        .env("PROVENANCE_STE100_ASSET_DIR", assets)
        .env("PROVENANCE_STE100_INDEX_DIR", indexes)
        .env("PROVENANCE_TEST_STE100_ASSET_URL", url)
        .args(init_args(repo))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap()
}

fn wait_until(event: &str, mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(120);
    while !done() {
        assert!(Instant::now() < deadline, "timed out: {event}");
        thread::sleep(Duration::from_millis(20));
    }
}
