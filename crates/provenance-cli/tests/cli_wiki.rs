use assert_cmd::Command;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::process::Command as StdCommand;
use std::time::{Duration, Instant};

#[test]
fn wiki_build_writes_static_pages_and_stylesheet() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    let repo = repo.to_string_lossy().to_string();
    let out = dir.path().join("site");
    seed_state(dir.path(), &repo);
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "wiki",
            "build",
            "--repo",
            &repo,
            "--out",
            &out.to_string_lossy(),
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains(r#""status": "ok""#))
        .stdout(predicates::str::contains(r#""scope": "default""#))
        .stdout(predicates::str::contains(
            r#""route": "/requirements/req_sah/""#,
        ));

    let index = std::fs::read_to_string(out.join("index.html")).unwrap();
    assert!(index.contains("Provenance Wiki"), "{index}");
    assert!(index.contains("role=\"search\""), "{index}");
    assert!(index.contains("href=\"/unfinished/\""), "{index}");
    assert!(index.contains("href=\"/decisions/\""), "{index}");

    let stylesheet = std::fs::read_to_string(out.join("assets/provenance-wiki.css")).unwrap();
    assert!(stylesheet.contains("--pv-"), "stylesheet missing tokens");
    assert!(out.join(".provenance-wiki-output.json").is_file());

    let domains = std::fs::read_to_string(out.join("domains/index.html")).unwrap();
    assert!(domains.contains("All requirements"), "{domains}");
    assert!(
        domains.contains("href=\"/requirements/req_sah/\""),
        "{domains}"
    );
    assert!(
        domains.contains("href=\"/rules/rule_sah_001/\""),
        "{domains}"
    );

    let search = std::fs::read_to_string(out.join("search/index.html")).unwrap();
    assert!(
        search.contains("Support at Home shall be traceable"),
        "{search}"
    );
    assert!(search.contains("Draft rule shall stay draft"), "{search}");
    assert!(search.contains("data-search-entry"), "{search}");
    assert!(!out.join("assets/search-index.json").exists());

    let requirement = std::fs::read_to_string(out.join("requirements/req_sah/index.html")).unwrap();
    assert!(
        requirement.contains("Support at Home shall be traceable"),
        "{requirement}"
    );
    assert!(requirement.contains("rule_sah_001"), "{requirement}");
    assert!(
        requirement.contains("href=\"/assets/provenance-wiki.css\""),
        "{requirement}"
    );

    let rule = std::fs::read_to_string(out.join("rules/rule_sah_001/index.html")).unwrap();
    assert!(
        rule.contains("No code scan was supplied to this build"),
        "{rule}"
    );
    assert!(!rule.contains("No function bound"), "{rule}");
    assert!(!rule.contains("Not verified"), "{rule}");

    let gapped = std::fs::read_to_string(out.join("requirements/req_gap/index.html")).unwrap();
    assert!(gapped.contains("citation gap"), "{gapped}");
}

#[test]
fn wiki_build_default_format_prints_a_concise_summary_not_a_page_dump() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    let repo = repo.to_string_lossy().to_string();
    let out = dir.path().join("site");
    seed_state(dir.path(), &repo);

    let output = Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "wiki",
            "build",
            "--repo",
            &repo,
            "--out",
            &out.to_string_lossy(),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).unwrap();

    assert!(!stdout.contains("\"pages\""), "{stdout}");
    assert!(!stdout.contains("\"route\""), "{stdout}");
    // 5 singleton pages + 2 requirements + 1 resolution + 1 rule + 1 source = 10 pages.
    assert!(
        stdout.contains("10 pages"),
        "expected the page count: {stdout}"
    );
    assert!(
        stdout.contains("wiki serve"),
        "expected a hint to view the site: {stdout}"
    );
    assert!(
        stdout.contains(&out.to_string_lossy().to_string()),
        "{stdout}"
    );
}

#[test]
fn wiki_build_refuses_an_unrecognized_custom_tree_without_partial_writes() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    let repo = repo.to_string_lossy().to_string();
    let out = dir.path().join("site");
    seed_state(dir.path(), &repo);

    // This is caller-owned content because no ownership marker recognizes it.
    std::fs::create_dir_all(out.join("requirements")).unwrap();
    std::fs::write(out.join("requirements/req_sah"), "blocking file").unwrap();

    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "wiki",
            "build",
            "--repo",
            &repo,
            "--out",
            &out.to_string_lossy(),
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "refusing nonempty custom wiki output",
        ));

    assert_eq!(
        std::fs::read_to_string(out.join("requirements/req_sah")).unwrap(),
        "blocking file"
    );
    assert!(!out.join("index.html").exists());
    assert!(!out.join("requirements/req_gap").exists());
    assert!(!out.join("assets").exists());
}

#[test]
fn wiki_build_defaults_output_to_the_provenance_wiki_dir_and_gitignores_it() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    let repo = repo.to_string_lossy().to_string();
    seed_state(dir.path(), &repo);

    Command::cargo_bin("provenance")
        .unwrap()
        .args(["wiki", "build", "--repo", &repo, "--format", "json"])
        .assert()
        .success()
        // Windows reports the default out with backslashes.
        .stdout(predicates::function::function(|text: &str| {
            text.replace("\\\\", "/").contains(".provenance/wiki")
        }));

    let default_out = std::path::Path::new(&repo).join(".provenance/wiki");
    let index = std::fs::read_to_string(default_out.join("index.html")).unwrap();
    assert!(index.contains("Provenance Wiki"), "{index}");

    let gitignore =
        std::fs::read_to_string(std::path::Path::new(&repo).join(".gitignore")).unwrap();
    assert!(
        gitignore
            .lines()
            .any(|line| line.trim() == ".provenance/wiki/"),
        "{gitignore}"
    );
}

#[test]
fn wiki_build_with_an_explicit_out_does_not_touch_gitignore() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    let repo = repo.to_string_lossy().to_string();
    let out = dir.path().join("site");
    seed_state(dir.path(), &repo);
    let gitignore = std::fs::read_to_string(std::path::Path::new(&repo).join(".gitignore"))
        .expect("init creates the cache ignore");

    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "wiki",
            "build",
            "--repo",
            &repo,
            "--out",
            &out.to_string_lossy(),
            "--format",
            "json",
        ])
        .assert()
        .success();

    assert_eq!(
        std::fs::read_to_string(std::path::Path::new(&repo).join(".gitignore")).unwrap(),
        gitignore,
        "explicit --out must not edit .gitignore"
    );
}

#[test]
fn wiki_build_accepts_an_absent_relative_custom_output() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    let repo = repo.to_string_lossy().to_string();
    seed_state(dir.path(), &repo);

    Command::cargo_bin("provenance")
        .unwrap()
        .current_dir(dir.path())
        .args(["wiki", "build", "--repo", &repo, "--out", "site"])
        .assert()
        .success();

    assert!(dir.path().join("site/index.html").is_file());
}

#[test]
fn wiki_build_creates_an_absent_custom_output_parent() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    let repo = repo.to_string_lossy().to_string();
    seed_state(dir.path(), &repo);

    Command::cargo_bin("provenance")
        .unwrap()
        .current_dir(dir.path())
        .args(["wiki", "build", "--repo", &repo, "--out", "dist/wiki"])
        .assert()
        .success();

    assert!(dir.path().join("dist/wiki/index.html").is_file());
}

#[test]
fn wiki_serve_serves_pages_stylesheet_and_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    let repo = repo.to_string_lossy().to_string();
    seed_state(dir.path(), &repo);

    let port = free_port();
    let mut child = StdCommand::new(assert_cmd::cargo::cargo_bin("provenance"))
        .args([
            "wiki",
            "serve",
            "--repo",
            &repo,
            "--host",
            "127.0.0.1",
            "--port",
            &port.to_string(),
        ])
        .spawn()
        .unwrap();

    let index = wait_for_http(port, "/");
    let stylesheet = wait_for_http(port, "/assets/provenance-wiki.css");
    let requirement = wait_for_http(port, "/requirements/req_sah/");
    let bare_route = wait_for_http(port, "/requirements/req_sah");
    let domains = wait_for_http(port, "/domains");
    let search = wait_for_http(port, "/search/");
    let unfinished = wait_for_http(port, "/unfinished/");
    let no_json_index = wait_for_http(port, "/assets/search-index.json");
    let missing = wait_for_http(port, "/nope/");
    child.kill().ok();
    child.wait().ok();

    assert!(index.contains("200 OK"), "{index}");
    assert!(index.contains("- Provenance Wiki</title>"), "{index}");
    assert!(index.contains("role=\"search\""), "{index}");

    assert!(stylesheet.contains("200 OK"), "{stylesheet}");
    assert!(stylesheet.contains("text/css"), "{stylesheet}");
    assert!(stylesheet.contains("--pv-"), "{stylesheet}");

    assert!(requirement.contains("200 OK"), "{requirement}");
    assert!(
        requirement.contains("Support at Home shall be traceable"),
        "{requirement}"
    );

    assert!(bare_route.contains("200 OK"), "{bare_route}");
    assert!(
        bare_route.contains("Support at Home shall be traceable"),
        "{bare_route}"
    );

    assert!(domains.contains("200 OK"), "{domains}");
    assert!(domains.contains("All requirements"), "{domains}");
    assert!(search.contains("200 OK"), "{search}");
    assert!(search.contains("id=\"wiki-search\""), "{search}");
    assert!(unfinished.contains("200 OK"), "{unfinished}");
    assert!(no_json_index.contains("404 Not Found"), "{no_json_index}");

    assert!(missing.contains("404 Not Found"), "{missing}");
    assert!(missing.contains("Page not found"), "{missing}");
}

#[allow(clippy::too_many_lines)]
fn seed_state(dir: &std::path::Path, repo: &str) {
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "init",
            "--path",
            repo,
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();

    let import_path = dir.join("state.json");
    std::fs::write(
        &import_path,
        r#"{
  "scope": "default",
  "sources": [{
    "schema_version": 1,
    "scope_id": "default",
    "id": "source_sah",
    "name": "Support at Home",
    "source_type": "legislation",
    "url": "https://example.test/sah",
    "reference": "Department guidance"
  }],
  "requirements": [{
    "schema_version": 1,
    "scope_id": "default",
    "id": "req_gap",
    "statement": "Uncited requirement",
    "status": "active",
    "source_refs": []
  }, {
    "schema_version": 1,
    "scope_id": "default",
    "id": "req_sah",
    "statement": "Support at Home shall be traceable",
    "status": "active",
    "source_refs": [{"source_id": "source_sah", "clause": "Program overview"}]
  }],
  "resolutions": [{
    "schema_version": 1,
    "scope_id": "default",
    "id": "res_sah",
    "title": "SAH extraction",
    "position": "Keep as draft extraction",
    "rationale": "Needs human review",
    "status": "approved",
    "requirement_ids": ["req_sah"],
    "review_on": null
  }],
  "rules": [{
    "schema_version": 1,
    "scope_id": "default",
    "id": "rule_sah_001",
    "name": "SAH rule",
    "statement": "Draft rule shall stay draft",
    "status": "active",
    "severity": "high",
    "requirement_ids": ["req_sah"],
    "resolution_ids": ["res_sah"],
    "source_document": "Example-API-main/src/example.php",
    "source_section": "lines 1-3"
  }],
  "edges": [],
  "threads": [],
  "messages": []
}"#,
    )
    .unwrap();

    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "import",
            "--repo",
            repo,
            "--scope",
            "default",
            "--input",
            &import_path.to_string_lossy(),
            "--format",
            "json",
        ])
        .assert()
        .success();
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
