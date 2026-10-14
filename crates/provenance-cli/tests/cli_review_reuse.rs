#[path = "review_host_support/mod.rs"]
mod review_host_support;

use provenance_macros::verifies;
use review_host_support::{
    events::Socket, launch_code, redeem, repository, request, start, BrowserRecorder, Host,
};
use serde_json::{json, Value};
use std::{
    path::Path,
    process::{Child, Command, Stdio},
    time::Duration,
};

fn create(root: &Path, args: &[&str]) {
    let output = Command::new(assert_cmd::cargo::cargo_bin("provenance"))
        .args(args)
        .args(["--repo", root.to_str().unwrap(), "--format", "json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// A Requirement with a Rule, and a host that serves it.
fn running_review() -> (tempfile::TempDir, Host) {
    let repo = repository();
    create(
        repo.path(),
        &[
            "req_reuse",
            "create",
            "--type",
            "requirement",
            "--statement",
            "The open page shows review links.",
        ],
    );
    create(
        repo.path(),
        &[
            "rule_reuse",
            "create",
            "--type",
            "rule",
            "--statement",
            "A review link selects its record in the open page.",
            "--requirement-id",
            "req_reuse",
        ],
    );
    let host = start(repo.path());
    (repo, host)
}

/// Starts `rule_reuse --review-link` with a recording browser.
fn review_link(host: &Host, root: &Path, browser: &BrowserRecorder, extra: &[&str]) -> Child {
    let mut command = host.cli();
    command
        .args(["rule_reuse", "--review-link", "--repo"])
        .arg(root)
        .args(["--format", "json"])
        .args(extra)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    browser.configure(&mut command);
    command.spawn().unwrap()
}

fn finish(child: Child) -> Value {
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn connected(host: &Host) -> Socket {
    let mut page = Socket::connect(host);
    assert_eq!(page.frame()["type"], "ready");
    page
}

fn record_page(host: &Host) -> String {
    format!(
        "{}/?root=req_reuse&focus=rule_reuse",
        host.config["endpoint"].as_str().unwrap()
    )
}

#[test]
#[verifies("rule_review_link_reuses_connected_page", examples)]
fn a_review_link_shows_its_record_in_the_connected_page() {
    let (repo, host) = running_review();
    let mut page = connected(&host);
    let browser = BrowserRecorder::install();
    let child = review_link(&host, repo.path(), &browser, &[]);

    let link = page.frame();
    assert_eq!(link["type"], "show-record");
    assert_eq!(link["rootId"], "req_reuse");
    assert_eq!(link["focusId"], "rule_reuse");
    page.send(&json!({"type": "ack", "requestId": link["requestId"]}));

    let output = finish(child);
    assert_eq!(output["opened"], true);
    assert_eq!(output["review_url"], record_page(&host));
    assert!(browser.opened_link().is_none());
}

#[test]
#[verifies("rule_review_link_reuses_connected_page", examples)]
fn the_newest_page_shows_the_link_and_only_it_can_acknowledge() {
    let (repo, host) = running_review();
    let mut older = connected(&host);
    let mut newer = connected(&host);
    let browser = BrowserRecorder::install();
    let mut child = review_link(&host, repo.path(), &browser, &[]);

    let link = newer.frame();
    older.send(&json!({"type": "ack", "requestId": link["requestId"]}));
    older.quiet();
    assert!(
        child.try_wait().unwrap().is_none(),
        "the older page cannot end the CLI wait"
    );
    newer.send(&json!({"type": "ack", "requestId": link["requestId"]}));

    assert_eq!(finish(child)["opened"], true);
    assert!(browser.opened_link().is_none());
}

#[test]
#[verifies("rule_review_commands_open_the_browser", examples)]
fn a_page_that_disconnects_before_it_acknowledges_leaves_the_link_to_the_browser() {
    let (repo, host) = running_review();
    let mut page = connected(&host);
    let browser = BrowserRecorder::install();
    let child = review_link(&host, repo.path(), &browser, &[]);
    assert_eq!(page.frame()["type"], "show-record");
    drop(page);

    let output = finish(child);
    assert_eq!(output["opened"], true);
    let opened = browser.wait_for_link(Duration::from_secs(15));
    assert!(opened.starts_with(&format!("{}#launch=", record_page(&host))));
    assert_eq!(redeem(&host, &launch_code(&opened)).status(), 200);
}

#[test]
/// Flow: --no-open keeps its printed link and does not move the connected page.
#[verifies("rule_review_link_printed_when_not_opened", examples)]
fn no_open_prints_the_link_without_moving_the_connected_page() {
    let (repo, host) = running_review();
    let mut page = connected(&host);
    let browser = BrowserRecorder::install();
    let output = finish(review_link(&host, repo.path(), &browser, &["--no-open"]));

    assert_eq!(output["opened"], false);
    assert!(output["review_url"]
        .as_str()
        .unwrap()
        .starts_with(&format!("{}#launch=", record_page(&host))));
    assert!(browser.opened_link().is_none());
    page.quiet();
}

#[test]
#[cfg(target_os = "linux")]
/// Implementation aid: a cancelled wait leaves no request that a later link must pass.
fn a_cancelled_review_link_does_not_block_the_next_one() {
    let (repo, host) = running_review();
    let mut page = connected(&host);
    let browser = BrowserRecorder::install();
    let sockets_before = host.socket_handles();
    let mut cancelled = review_link(&host, repo.path(), &browser, &[]);
    let first = page.frame();
    let waiting_sockets = host.socket_handles();
    let request_sockets: Vec<_> = waiting_sockets
        .difference(&sockets_before)
        .cloned()
        .collect();
    assert!(
        !request_sockets.is_empty(),
        "the host holds the waiting HTTP request"
    );
    cancelled.kill().unwrap();
    cancelled.wait().unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while request_sockets
        .iter()
        .any(|socket| host.socket_handles().contains(socket))
    {
        assert!(
            std::time::Instant::now() < deadline,
            "the host kept the cancelled HTTP handler"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
    page.send(&json!({"type": "ack", "requestId": first["requestId"]}));

    let child = review_link(&host, repo.path(), &browser, &[]);
    let second = page.frame();
    assert_ne!(second["requestId"], first["requestId"]);
    page.send(&json!({"type": "ack", "requestId": second["requestId"]}));
    assert_eq!(finish(child)["opened"], true);
    assert!(browser.opened_link().is_none());
}

#[test]
/// Implementation aid: only the launch key holder can move a connected page.
fn navigation_requires_the_launch_key() {
    let (_repo, host) = running_review();
    let mut page = connected(&host);
    let refused = request(&host, "POST", "/review-navigation", true)
        .send_string(&json!({"launchKey": "0".repeat(64), "rootId": "req_reuse"}).to_string());
    let Err(ureq::Error::Status(status, _)) = refused else {
        panic!("navigation without the launch key must be refused");
    };
    assert_eq!(status, 401);
    page.quiet();
}
