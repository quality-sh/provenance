use assert_cmd::Command;
use provenance_core::SUPPORTED_SCHEMA_VERSION;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::process::Command as StdCommand;
use std::time::{Duration, Instant};

#[test]
fn wiki_build_uses_coverage_report_for_implementations_and_verifications() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    let out = dir.path().join("site");
    let report = dir.path().join("coverage.json");
    seed_rules(dir.path(), &repo);
    seed_git_remote(&repo);
    write_coverage_report(&report);

    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "wiki",
            "build",
            "--repo",
            &repo.to_string_lossy(),
            "--out",
            &out.to_string_lossy(),
            "--coverage",
            &report.to_string_lossy(),
        ])
        .assert()
        .success();

    let bound = std::fs::read_to_string(out.join("rules/rule_bound/index.html")).unwrap();
    assert!(bound.contains("Implementation"), "{bound}");
    assert!(!bound.contains("Rule Function"), "{bound}");
    assert!(bound.contains("decide_bound_rule"), "{bound}");
    assert!(bound.contains("src/rules.rs:7"), "{bound}");
    assert!(
        bound.contains("https://github.com/example/provenance/blob/abc1234/src/rules.rs#L7"),
        "{bound}"
    );
    assert!(bound.contains("Verification"), "{bound}");
    assert!(bound.contains("exhaustion"), "{bound}");
    assert!(bound.contains("bound_rule_exhaustion"), "{bound}");
    assert!(bound.contains("examples"), "{bound}");
    assert!(bound.contains("bound_rule_examples"), "{bound}");
    assert!(bound.contains("tests/rules.rs:12"), "{bound}");
    assert!(bound.contains("Local snippet"), "{bound}");
    assert!(bound.contains("fn decide_bound_rule() {}"), "{bound}");
    assert!(!bound.contains("/blob/HEAD/"), "{bound}");
    assert_eq!(
        bound.matches("outside implementation module").count(),
        1,
        "{bound}"
    );
    assert!(!bound.contains("docs/obsolete.md"), "{bound}");
    assert!(!bound.contains(">Evidence</h2>"), "{bound}");

    assert!(
        bound.contains("Code scan at commit <code>abc1234</code>"),
        "{bound}"
    );

    let unbound = std::fs::read_to_string(out.join("rules/rule_unbound/index.html")).unwrap();
    assert!(unbound.contains("No implementation bound"), "{unbound}");
    assert!(unbound.contains("Not verified"), "{unbound}");
    assert!(
        unbound.contains("Code scan at commit <code>abc1234</code>"),
        "{unbound}"
    );
}

#[test]
fn wiki_build_without_scan_suppresses_unpinned_code_links() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    let out = dir.path().join("site");
    seed_rules(dir.path(), &repo);
    seed_git_remote(&repo);

    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "wiki",
            "build",
            "--repo",
            &repo.to_string_lossy(),
            "--out",
            &out.to_string_lossy(),
        ])
        .assert()
        .success();

    let source = std::fs::read_to_string(out.join("sources/source_code/index.html")).unwrap();
    assert!(source.contains("src/rules.rs:7"), "{source}");
    assert!(!source.contains("/blob/HEAD/"), "{source}");
    assert!(
        !source.contains("href=\"https://github.com/example/provenance/blob/"),
        "{source}"
    );
}

#[test]
fn wiki_build_without_scan_renders_canonical_typed_implementation() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    let out = dir.path().join("site");
    seed_rules(dir.path(), &repo);
    seed_implementation_binding(&repo);
    seed_git_remote(&repo);

    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "wiki",
            "build",
            "--repo",
            &repo.to_string_lossy(),
            "--out",
            &out.to_string_lossy(),
        ])
        .assert()
        .success();

    let rule = std::fs::read_to_string(out.join("rules/rule_bound/index.html")).unwrap();
    assert!(rule.contains(">Implementation</h2>"), "{rule}");
    assert!(rule.contains("decide_typed_rule"), "{rule}");
    assert!(rule.contains("src/typed-rules.ts"), "{rule}");
    assert!(rule.contains("No code scan was supplied"), "{rule}");
    assert!(!rule.contains("/blob/HEAD/"), "{rule}");
}

#[test]
fn wiki_serve_without_coverage_omits_scan_sections_and_unpinned_links() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    seed_rules(dir.path(), &repo);
    seed_git_remote(&repo);

    let port = free_port();
    let mut child = spawn_wiki_server(&repo, port, None);
    let rule = wait_for_http(port, "/rules/rule_bound/");
    let source = wait_for_http(port, "/sources/source_code/");
    child.kill().ok();
    child.wait().ok();

    assert!(rule.contains("200 OK"), "{rule}");
    assert!(rule.contains("No code scan was supplied"), "{rule}");
    assert!(!rule.contains("Implementation"), "{rule}");
    assert!(source.contains("src/rules.rs:7"), "{source}");
    assert!(!source.contains("/blob/HEAD/"), "{source}");
}

#[test]
fn wiki_serve_with_coverage_renders_pins_snippets_and_implementations() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    let report = dir.path().join("coverage.json");
    seed_rules(dir.path(), &repo);
    seed_git_remote(&repo);
    write_coverage_report(&report);

    let port = free_port();
    let mut child = spawn_wiki_server(&repo, port, Some(&report));
    let rule = wait_for_http(port, "/rules/rule_bound/");
    child.kill().ok();
    child.wait().ok();

    assert!(rule.contains("200 OK"), "{rule}");
    assert!(
        rule.contains("Code scan at commit <code>abc1234</code>"),
        "{rule}"
    );
    assert!(rule.contains("Implementation"), "{rule}");
    assert!(!rule.contains("Rule Function"), "{rule}");
    assert!(rule.contains("decide_bound_rule"), "{rule}");
    assert!(rule.contains("Verification"), "{rule}");
    assert!(rule.contains("exhaustion"), "{rule}");
    assert!(rule.contains("bound_rule_exhaustion"), "{rule}");
    assert!(rule.contains("examples"), "{rule}");
    assert!(rule.contains("bound_rule_examples"), "{rule}");
    assert!(rule.contains("tests/rules.rs:12"), "{rule}");
    assert!(rule.contains("Local snippet"), "{rule}");
    assert!(rule.contains("fn decide_bound_rule() {}"), "{rule}");
    assert!(
        rule.contains("https://github.com/example/provenance/blob/abc1234/src/rules.rs#L7"),
        "{rule}"
    );
    assert!(!rule.contains("/blob/HEAD/"), "{rule}");
}

fn write_coverage_report(path: &std::path::Path) {
    std::fs::write(
        path,
        r#"{
  "commit": "abc1234",
  "files_scanned": 2,
  "total_annotations": 0,
  "warnings": [],
  "annotations": [],
  "scanned_files": [{
    "file_path": "src/rules.rs",
    "content": "line one\nline two\nline three\nline four\nline five\nline six\nfn decide_bound_rule() {}\nline eight\nline nine\nline ten\nline eleven\nline twelve\nline thirteen\nline fourteen\nline fifteen\nline sixteen\nline seventeen\nline eighteen\nline nineteen\nline twenty\nline twenty-one\nline twenty-two\nline twenty-three\nline twenty-four\nline twenty-five\nline twenty-six\nline twenty-seven\nline twenty-eight\nline twenty-nine\nline thirty\nfn bound_rule_exhaustion() {}\n"
  }, {
    "file_path": "tests/rules.rs",
    "content": "test line one\ntest line two\ntest line three\ntest line four\ntest line five\ntest line six\ntest line seven\ntest line eight\ntest line nine\ntest line ten\ntest line eleven\nfn bound_rule_examples() {}\n"
  }],
  "bindings": [{
    "rule_id": "rule_bound",
    "file_path": "src/rules.rs",
    "line": 7,
    "item_name": "decide_bound_rule",
    "verification": null
  }, {
    "rule_id": "rule_bound",
    "file_path": "src/rules.rs",
    "line": 31,
    "item_name": "bound_rule_exhaustion",
    "verification": "exhaustion"
  }, {
    "rule_id": "rule_bound",
    "file_path": "tests/rules.rs",
    "line": 12,
    "item_name": "bound_rule_examples",
    "verification": "examples"
  }]
}"#,
    )
    .unwrap();
}

fn seed_rules(dir: &std::path::Path, repo: &std::path::Path) {
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "init",
            "--path",
            &repo.to_string_lossy(),
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();
    let state = dir.join("state.json");
    std::fs::write(
        &state,
        serde_json::json!({
          "scope": "default",
          "sources": [{
            "schema_version": SUPPORTED_SCHEMA_VERSION.0,
            "scope_id": "default",
            "id": "source_code",
            "name": "Code reference",
            "source_type": "project_artifact",
            "reference": "src/rules.rs:7"
          }],
          "requirements": [{
            "schema_version": SUPPORTED_SCHEMA_VERSION.0,
            "scope_id": "default",
            "id": "req_code",
            "statement": "The code reference shall be honoured.",
            "status": "active",
            "source_refs": [{"source_id": "source_code", "clause": null}]
          }],
          "resolutions": [],
          "rules": [{
            "schema_version": SUPPORTED_SCHEMA_VERSION.0,
            "scope_id": "default",
            "id": "rule_bound",
            "name": "Bound rule",
            "statement": "The bound decision is canonical.",
            "status": "active",
            "severity": "high",
            "requirement_ids": ["req_code"],
            "source_document": "docs/obsolete.md",
            "source_section": "old_description"
          }, {
            "schema_version": SUPPORTED_SCHEMA_VERSION.0,
            "scope_id": "default",
            "id": "rule_unbound",
            "name": "Unbound rule",
            "statement": "An absent binding is shown honestly.",
            "status": "active",
            "severity": "medium",
            "requirement_ids": ["req_code"]
          }],
          "threads": [],
          "messages": []
        })
        .to_string(),
    )
    .unwrap();
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "import",
            "--repo",
            &repo.to_string_lossy(),
            "--scope",
            "default",
            "--input",
            &state.to_string_lossy(),
            "--format",
            "json",
        ])
        .assert()
        .success();
}

fn seed_implementation_binding(repo: &std::path::Path) {
    let shard = repo.join(".provenance/state/scopes/default/implementations/binding.jsonl");
    std::fs::create_dir_all(shard.parent().unwrap()).unwrap();
    std::fs::write(
        shard,
        serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0,"scope_id":"default","id":"implementation_binding_rule_bound","rule_id":"rule_bound","declared_by":"spec://typescript/payroll","file":"src/typed-rules.ts","symbol":"decide_typed_rule"}).to_string(),
    )
    .unwrap();
}

fn seed_git_remote(repo: &std::path::Path) {
    std::process::Command::new("git")
        .args(["init", repo.to_str().unwrap()])
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args([
            "-C",
            repo.to_str().unwrap(),
            "remote",
            "add",
            "origin",
            "git@github.com:example/provenance.git",
        ])
        .output()
        .unwrap();
}

fn spawn_wiki_server(
    repo: &std::path::Path,
    port: u16,
    coverage: Option<&std::path::Path>,
) -> std::process::Child {
    let mut command = StdCommand::new(assert_cmd::cargo::cargo_bin("provenance"));
    command.args([
        "wiki",
        "serve",
        "--repo",
        repo.to_str().unwrap(),
        "--host",
        "127.0.0.1",
        "--port",
        &port.to_string(),
    ]);
    if let Some(coverage) = coverage {
        command.args(["--coverage", coverage.to_str().unwrap()]);
    }
    command.spawn().unwrap()
}

fn free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

fn wait_for_http(port: u16, path: &str) -> String {
    let deadline = Instant::now() + Duration::from_secs(3);
    let addr: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    let mut last_error = None;
    while Instant::now() < deadline {
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
