use super::{directory_file, envelope, init, provenance, refusal};
use serde_json::{json, Value};
use std::io::Read as _;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[test]
fn api_rejects_unsupported_methods_and_malformed_flags_before_any_call() {
    let (_directory, repo) = init();

    provenance()
        .args(["api", "sources", "--repo", &repo, "--method", "delete"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("invalid value"));

    provenance()
        .args(["api", "sources", "--repo", &repo, "--header", "NoSeparator"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("NAME: VALUE"));

    provenance()
        .args(["api", "requirements", "--repo", &repo, "--query", "limit"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("NAME=VALUE"));

    provenance()
        .args([
            "api",
            "requirements",
            "--repo",
            &repo,
            "--method",
            "get",
            "--method",
            "post",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains("cannot be used multiple times"));

    let body = directory_file(
        &repo,
        "body.json",
        json!({"name": "X"}).to_string().as_str(),
    );
    let refused = refusal(&[
        "api", "sources", "--repo", &repo, "--method", "get", "--input", &body,
    ]);
    assert_eq!(refused["error"]["kind"], "invalid_input");
    assert_eq!(refused["error"]["field"], "body");
}

#[test]
fn api_refuses_repeated_header_and_query_names() {
    let (_directory, repo) = init();

    for (flag, first, second, field) in [
        ("--header", "Accept: one", "Accept: two", "headers"),
        ("--header", "If-Match: \"1\"", "if-match: \"2\"", "headers"),
        ("--query", "limit=1", "limit=2", "query"),
    ] {
        let refused = refusal(&[
            "api",
            "requirements",
            "--repo",
            &repo,
            flag,
            first,
            flag,
            second,
        ]);
        assert_eq!(refused["error"]["kind"], "invalid_input", "{first}");
        assert_eq!(refused["error"]["field"], field, "{first}");
    }
}

#[test]
fn api_method_without_a_path_refuses_instead_of_discovering() {
    let (_directory, repo) = init();

    let refused = refusal(&["api", "--method", "post", "--repo", &repo]);
    assert_eq!(refused["error"]["kind"], "invalid_input");
    assert_eq!(refused["error"]["field"], "method");
}

/// Run one api command with standard input held open and never written,
/// and return its refusal; a command that waits on the input fails the test.
fn refusal_with_open_stdin(arguments: &[&str]) -> Value {
    let mut child = Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let held = child.stdin.take();
    let deadline = Instant::now() + Duration::from_secs(30);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() > deadline {
            child.kill().unwrap();
            panic!("api waited on standard input: {arguments:?}");
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    drop(held);
    assert!(!status.success(), "{arguments:?}");
    let mut stderr = String::new();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .unwrap();
    envelope(&stderr)
}

#[test]
fn api_refuses_a_stdin_body_before_reading_it() {
    let (_directory, repo) = init();

    let repo = repo.as_str();
    for arguments in [
        vec!["api", "--input", "-", "--repo", repo],
        vec!["api", "sources", "--input", "-", "--repo", repo],
        vec![
            "api", "sources", "--method", "GET", "--input", "-", "--repo", repo,
        ],
    ] {
        let refused = refusal_with_open_stdin(&arguments);
        assert_eq!(refused["error"]["kind"], "invalid_input", "{arguments:?}");
        assert_eq!(refused["error"]["field"], "body", "{arguments:?}");
    }
}
