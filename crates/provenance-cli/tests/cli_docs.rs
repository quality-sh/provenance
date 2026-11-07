use assert_cmd::Command;
use provenance_macros::verifies;
use serde_json::Value;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::process::Command as StdCommand;
use std::time::{Duration, Instant};

/// Runs `check --docs --format json` and returns whether it passed and the docs category.
fn check_docs(repo: &std::path::Path) -> (bool, Value) {
    let output = Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "check",
            "--docs",
            "--repo",
            &repo.to_string_lossy(),
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    let categories = report["categories"].as_array().unwrap();
    assert_eq!(categories.len(), 1, "{report}");
    assert_eq!(categories[0]["category"], "docs");
    (output.status.success(), categories[0].clone())
}

// The link decision is checked against generated sites in
// provenance-transport/src/docs/tests.rs. These cases pin it to the command a
// reader runs: the links below are the ones a real docs tree is written with,
// and `check --docs` has to pass or fail on them through the whole
// load-and-resolve path, not just in the decision function.
#[test]
#[verifies("rule_docs_links_resolve", examples)]
fn check_docs_accepts_plain_markdown_tree() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    std::fs::create_dir_all(repo.join("docs/guide")).unwrap();
    std::fs::write(
        repo.join("docs/index.md"),
        "# Agent Handbook\n\nSee [Install](guide/install.md).\n",
    )
    .unwrap();
    std::fs::write(
        repo.join("docs/guide/install.md"),
        "# Installation\n\nBack to [home](../index.md).\n",
    )
    .unwrap();

    let (passed, docs) = check_docs(repo);
    assert!(passed, "{docs}");
    assert_eq!(docs["status"], "passed");
    assert_eq!(docs["findings"], serde_json::json!([]));
}

#[test]
#[verifies("rule_docs_links_resolve", examples)]
fn check_docs_fails_on_broken_relative_markdown_links() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    std::fs::create_dir_all(repo.join("docs")).unwrap();
    std::fs::write(
        repo.join("docs/index.md"),
        "# Agent Handbook\n\nSee [Missing](missing.md).\n",
    )
    .unwrap();

    let (passed, docs) = check_docs(repo);
    assert!(!passed, "{docs}");
    assert_eq!(docs["status"], "findings");
    assert_eq!(docs["findings"].as_array().unwrap().len(), 1);
    let detail = &docs["findings"][0]["detail"];
    assert_eq!(detail["source_path"], "docs/index.md");
    assert_eq!(detail["link"], "missing.md");
}

// `check --docs` and `docs serve` read a page with the same parser options,
// so a footnote is a footnote to both. Read without footnotes, the block
// below is a link definition and `[^1]` the link using it, and the command
// would fail on a page the server renders with no link on it at all.
#[test]
#[verifies("rule_docs_links_resolve", examples)]
fn check_docs_reads_footnotes_the_way_the_server_renders_them() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    std::fs::create_dir_all(repo.join("docs")).unwrap();
    std::fs::write(
        repo.join("docs/index.md"),
        "# Agent Handbook\n\nThe handbook is versioned.[^1]\n\n[^1]: guide/missing.md\n",
    )
    .unwrap();

    let (passed, docs) = check_docs(repo);
    assert!(passed, "{docs}");
    assert_eq!(docs["findings"], serde_json::json!([]));
}

// A link out of docs/ and back into the repo root resolves only when the
// site publishes README.md as a page.
#[test]
#[verifies("rule_docs_links_resolve", examples)]
fn check_docs_uses_readme_as_homepage_when_docs_index_is_absent() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    std::fs::create_dir_all(repo.join("docs/guide")).unwrap();
    std::fs::write(
        repo.join("README.md"),
        "# Project Handbook\n\nContinue to [Install](docs/guide/install.md).\n",
    )
    .unwrap();
    std::fs::write(
        repo.join("docs/guide/install.md"),
        "# Installation\n\nBack to [home](../../README.md).\n",
    )
    .unwrap();

    let (passed, docs) = check_docs(repo);
    assert!(passed, "{docs}");
    assert_eq!(docs["status"], "passed");
}

#[test]
fn docs_serve_renders_semantic_html_and_rewrites_markdown_links() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    std::fs::create_dir_all(repo.join("docs/guide")).unwrap();
    std::fs::write(
        repo.join("docs/index.md"),
        "# Agent Handbook\n\nSee [Install](guide/install.md).\n",
    )
    .unwrap();
    std::fs::write(repo.join("docs/guide/install.md"), "# Installation\n").unwrap();

    let port = free_port();
    let mut child = StdCommand::new(assert_cmd::cargo::cargo_bin("provenance"))
        .args([
            "docs",
            "serve",
            "--repo",
            &repo.to_string_lossy(),
            "--host",
            "127.0.0.1",
            "--port",
            &port.to_string(),
        ])
        .spawn()
        .unwrap();

    let response = wait_for_http(port, "/");
    child.kill().ok();
    child.wait().ok();

    assert!(response.contains("200 OK"), "{response}");
    assert!(response.contains("<title>Agent Handbook - Provenance Docs</title>"));
    assert!(response.contains("<nav aria-label=\"Docs navigation\">"));
    assert!(response.contains("<main>"));
    assert!(response.contains("<article>"));
    assert!(response.contains("href=\"/guide/install/\""));
    assert!(!response.contains("href=\"guide/install.md\""));
    assert!(!response.contains("class="));
}

fn free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

fn wait_for_http(port: u16, path: &str) -> String {
    let deadline = Instant::now() + Duration::from_secs(10);
    let addr: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    let mut last_error = None;

    while Instant::now() < deadline {
        // Treat connect, write, and read as a single attempt: a listener
        // that has only just bound (or is still behind the spawning
        // process's startup) can accept the TCP handshake and then reset
        // the connection before it is actually ready to serve a full
        // request/response cycle. Retrying the whole attempt on any IO
        // error here (not just on connection refused) is what makes this
        // robust against that startup race instead of panicking on it.
        match attempt_http_request(addr, path) {
            Ok(response) => return response,
            Err(error) => {
                last_error = Some(error);
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }

    panic!("server did not respond: {last_error:?}");
}

fn attempt_http_request(addr: SocketAddr, path: &str) -> std::io::Result<String> {
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_millis(150))?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n"
    )?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    Ok(response)
}
