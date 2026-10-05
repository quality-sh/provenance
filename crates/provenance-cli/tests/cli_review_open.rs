#[path = "review_host_support/mod.rs"]
mod review_host_support;

use provenance_macros::verifies;
use review_host_support::{
    launch_code, redeem, repository, start, start_with, BrowserRecorder, Host,
};
use serde_json::Value;
use std::{path::Path, process::Command, time::Duration};

fn create_requirement(root: &Path) {
    let output = Command::new(assert_cmd::cargo::cargo_bin("provenance"))
        .args([
            "req_open",
            "create",
            "--type",
            "requirement",
            "--repo",
            root.to_str().unwrap(),
            "--statement",
            "The agent gives the reviewer a link.",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Runs `--review-link --format json` with the given changes to the command.
fn review_link(host: &Host, root: &Path, configure: impl FnOnce(&mut Command)) -> Value {
    let mut command = host.cli();
    command.args([
        "req_open",
        "--review-link",
        "--repo",
        root.to_str().unwrap(),
        "--format",
        "json",
    ]);
    configure(&mut command);
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn record_page(host: &Host) -> String {
    format!(
        "{}/?root=req_open",
        host.config["endpoint"].as_str().unwrap()
    )
}

fn running_review() -> (tempfile::TempDir, Host) {
    let repo = repository();
    create_requirement(repo.path());
    let host = start(repo.path());
    (repo, host)
}

#[test]
/// This flow opens the link through the BROWSER program and redeems the code it received.
#[verifies("rule_review_commands_open_the_browser", examples)]
fn review_link_opens_the_record_in_the_browser() {
    let (repo, host) = running_review();
    let browser = BrowserRecorder::install();
    let output = review_link(&host, repo.path(), |command| browser.configure(command));

    assert_eq!(output["opened"], true);
    assert_eq!(output["review_url"], record_page(&host));
    let opened = browser.opened_link().unwrap();
    assert!(opened.starts_with(&format!("{}#launch=", record_page(&host))));
    assert_eq!(redeem(&host, &launch_code(&opened)).status(), 200);
}

#[test]
/// This flow starts the host, which opens the bare review page signed in.
#[verifies("rule_review_commands_open_the_browser", examples)]
fn review_opens_the_page_at_startup() {
    let repo = repository();
    let browser = BrowserRecorder::install();
    let host = start_with(repo.path(), |command| browser.configure(command));

    let opened = browser.wait_for_link(Duration::from_secs(15));
    let page = format!("{}/#launch=", host.config["endpoint"].as_str().unwrap());
    assert!(opened.starts_with(&page));
    assert_eq!(redeem(&host, &launch_code(&opened)).status(), 200);
}

#[test]
#[verifies("rule_review_commands_open_the_browser", examples)]
fn no_open_does_not_run_the_browser() {
    let (repo, host) = running_review();
    let browser = BrowserRecorder::install();
    let output = review_link(&host, repo.path(), |command| {
        browser.configure(command);
        command.arg("--no-open");
    });

    assert!(browser.opened_link().is_none());
    assert_eq!(output["opened"], false);
    let link = output["review_url"].as_str().unwrap();
    assert!(link.starts_with(&format!("{}#launch=", record_page(&host))));
}

#[test]
#[verifies("rule_review_link_printed_when_not_opened", examples)]
fn an_ssh_session_prints_the_link() {
    let (repo, host) = running_review();
    let browser = BrowserRecorder::install();
    let output = review_link(&host, repo.path(), |command| {
        browser.configure(command);
        command.env("SSH_CONNECTION", "192.0.2.1 50000 192.0.2.2 22");
    });

    assert!(browser.opened_link().is_none());
    assert_eq!(output["opened"], false);
    let link = output["review_url"].as_str().unwrap();
    assert_eq!(redeem(&host, &launch_code(link)).status(), 200);
}

#[test]
#[verifies("rule_review_link_printed_when_not_opened", examples)]
fn a_failed_browser_prints_the_link() {
    let (repo, host) = running_review();
    let browser = BrowserRecorder::install();
    let missing = repo.path().join("no-such-browser");
    let output = review_link(&host, repo.path(), |command| {
        browser.configure(command);
        command.env("BROWSER", &missing);
    });

    assert_eq!(output["opened"], false);
    let link = output["review_url"].as_str().unwrap();
    assert_eq!(redeem(&host, &launch_code(link)).status(), 200);
}
