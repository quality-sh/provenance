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
#[verifies("rule_ste_dictionary_download_lock_scope", examples)]
fn a_failing_download_does_not_block_a_concurrent_init() {
    let _serial = serial();
    let failing = TestServer::held(503, b"unavailable");
    let working = TestServer::new(200, dictionary_support::dictionary_pdf());
    let temporary = tempfile::tempdir().unwrap();
    let assets = temporary.path().join("assets");
    let indexes = temporary.path().join("indexes");
    let slow_repo = temporary.path().join("slow");
    let fast_repo = temporary.path().join("fast");
    seed_the_dictionary_index(temporary.path(), &indexes);

    // The held request of the first init fails only after the second init
    // has put its PDF in the cache. With the index in place, the second init
    // does not parse the PDF, so it publishes well inside the client timeout
    // of the first init.
    let slow = spawn_init(&slow_repo, &assets, &indexes, &failing.url());
    wait_until("the first init requests the asset", || {
        !failing.requests().is_empty()
    });
    let fast = spawn_init(&fast_repo, &assets, &indexes, &working.url());
    wait_until("the second init puts its PDF in the cache", || {
        assets.join("ASD-STE100_ISSUE9.pdf").is_file()
    });
    failing.release();
    let fast = fast.wait_with_output().unwrap();
    let slow = slow.wait_with_output().unwrap();

    assert!(
        fast.status.success(),
        "{}",
        String::from_utf8_lossy(&fast.stderr)
    );
    assert!(dictionary_support::reference_path(&fast_repo).is_file());
    assert_eq!(working.requests().len(), 1);
    let slow_stderr = String::from_utf8_lossy(&slow.stderr);
    assert!(slow.status.success(), "{slow_stderr}");
    assert!(
        !slow_stderr.contains("dictionary could not be added"),
        "the second init waited for the failing download of the first init: {slow_stderr}"
    );
    assert!(
        dictionary_support::reference_path(&slow_repo).is_file(),
        "the first init uses the dictionary that the second init put in the cache"
    );
}

/// Imports a local PDF into the shared index directory, so that a later init
/// loads the index and does not parse the PDF again.
fn seed_the_dictionary_index(root: &Path, indexes: &Path) {
    let pdf = root.join("issue-9.pdf");
    std::fs::write(&pdf, dictionary_support::dictionary_pdf()).unwrap();
    let mut args = init_args(&root.join("seed"));
    args.extend(["--ste-pdf".to_owned(), pdf.to_str().unwrap().to_owned()]);
    let output = std::process::Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
        .env("PROVENANCE_STE100_ASSET_DIR", root.join("seed-assets"))
        .env("PROVENANCE_STE100_INDEX_DIR", indexes)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
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
