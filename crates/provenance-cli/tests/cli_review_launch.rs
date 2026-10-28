#[path = "review_host_support/mod.rs"]
mod review_host_support;

use provenance_macros::verifies;
use provenance_transport::local_host::LAUNCH_CODE_ROUTE;
use review_host_support::{launch_code, redeem, repository, request, response, start, Host};
use serde_json::{json, Value};
use std::path::Path;

fn create_requirement(root: &Path) {
    let output = std::process::Command::new(assert_cmd::cargo::cargo_bin("provenance"))
        .args([
            "req_launch",
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

fn review_link(host: &Host, root: &Path) -> String {
    let output = host
        .cli()
        .args([
            "req_launch",
            "--review-link",
            "--no-open",
            "--repo",
            root.to_str().unwrap(),
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
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    value["review_url"].as_str().unwrap().to_owned()
}

fn started_review() -> (tempfile::TempDir, Host, String) {
    let repo = repository();
    create_requirement(repo.path());
    let host = start(repo.path());
    let link = review_link(&host, repo.path());
    (repo, host, link)
}

fn issue_code(host: &Host, launch_key: &str) -> ureq::Response {
    response(
        request(host, "POST", LAUNCH_CODE_ROUTE, false)
            .set("Content-Type", "application/json")
            .send_string(&json!({ "launchKey": launch_key }).to_string()),
    )
}

#[test]
/// This flow runs the real host and CLI, redeems the link code, and reads with the session.
#[verifies("rule_review_link_opens_signed_in", examples)]
fn review_link_opens_the_page_signed_in() {
    let (_repo, host, link) = started_review();
    let page = format!(
        "{}/?root=req_launch#launch=",
        host.config["endpoint"].as_str().unwrap()
    );
    assert!(link.starts_with(&page));

    let session = redeem(&host, &launch_code(&link));
    assert_eq!(session.status(), 200);
    let session: Value = serde_json::from_str(&session.into_string().unwrap()).unwrap();
    let session = session["session"].as_str().unwrap();
    assert_ne!(session, host.bearer());
    let configuration = response(
        request(&host, "GET", "/review-config", false)
            .set("Authorization", &format!("Bearer {session}"))
            .call(),
    );
    assert_eq!(configuration.status(), 200);
}

#[test]
#[verifies("rule_review_launch_code_lifetime", examples)]
fn a_launch_code_works_once() {
    let (_repo, host, link) = started_review();
    let code = launch_code(&link);
    assert_eq!(redeem(&host, &code).status(), 200);
    assert_eq!(redeem(&host, &code).status(), 401);
}

#[test]
/// Implementation aid: security hardening keeps a launch code bound to the host that issued it.
fn another_host_refuses_the_code() {
    let (_repo, _host, link) = started_review();
    let other_repo = repository();
    let other_host = start(other_repo.path());
    assert_eq!(redeem(&other_host, &launch_code(&link)).status(), 401);
}

#[test]
/// Implementation aid: security hardening issues launch codes only to the launch key holder.
fn minting_needs_the_launch_key() {
    let repo = repository();
    let host = start(repo.path());
    let wrong_key = "0".repeat(64);
    assert_eq!(issue_code(&host, &wrong_key).status(), 401);
}

#[test]
/// Implementation aid: security hardening keeps the launch key out of every host response.
fn no_route_returns_the_launch_key() {
    let repo = repository();
    let host = start(repo.path());
    let launch_key = std::fs::read_to_string(host.launch_key_path()).unwrap();
    assert!(!host.config.to_string().contains(&launch_key));
    let mut bodies = Vec::new();
    for (path, auth) in [
        ("/", false),
        ("/local-host-identity", false),
        ("/metadata", true),
        ("/review-config", true),
    ] {
        bodies.push(
            response(request(&host, "GET", path, auth).call())
                .into_string()
                .unwrap(),
        );
    }
    let issued = issue_code(&host, &launch_key).into_string().unwrap();
    let code = serde_json::from_str::<Value>(&issued).unwrap()["code"]
        .as_str()
        .unwrap()
        .to_owned();
    bodies.push(issued);
    bodies.push(redeem(&host, &code).into_string().unwrap());
    for body in bodies {
        assert!(!body.contains(&launch_key));
    }
}

#[test]
#[verifies("rule_review_link_printed_when_not_opened", examples)]
fn a_missing_launch_key_prints_the_plain_link() {
    let repo = repository();
    create_requirement(repo.path());
    let host = start(repo.path());
    let plain = format!(
        "{}/?root=req_launch",
        host.config["endpoint"].as_str().unwrap()
    );
    // Another per-user cache, as an agent sandbox has, holds no launch key for this host.
    let other_cache = tempfile::tempdir().unwrap();
    let mut command = std::process::Command::new(assert_cmd::cargo::cargo_bin("provenance"));
    command
        .env("HOME", other_cache.path())
        .env("XDG_CACHE_HOME", other_cache.path().join("cache"))
        .env("LOCALAPPDATA", other_cache.path().join("cache"))
        .env(
            "PROVENANCE_TEST_STE100_ASSET_URL",
            env!("PROVENANCE_TEST_STE100_ASSET_URL"),
        )
        .args([
            "req_launch",
            "--review-link",
            "--no-open",
            "--repo",
            repo.path().to_str().unwrap(),
        ]);

    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), plain);
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("warning: the review link cannot sign the page in"));

    let output = command.args(["--format", "json"]).output().unwrap();
    assert!(output.status.success());
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["review_url"], plain);
    assert_eq!(value["opened"], false);
}
