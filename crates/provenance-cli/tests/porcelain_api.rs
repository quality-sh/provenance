use assert_cmd::Command;
use provenance_macros::verifies;
use serde_json::{json, Value};

fn provenance() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
}

fn init() -> (tempfile::TempDir, String) {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_string_lossy().into_owned();
    provenance()
        .args([
            "init",
            "--path",
            &repo,
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();
    (directory, repo)
}

fn seed_source(repo: &str) {
    provenance()
        .args([
            "sources",
            "create",
            "--repo",
            repo,
            "--id",
            "source_api",
            "--name",
            "Shared source",
        ])
        .assert()
        .success();
}

fn output(arguments: &[&str]) -> std::process::Output {
    provenance().args(arguments).output().unwrap()
}

fn success(arguments: &[&str]) -> std::process::Output {
    let result = output(arguments);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    result
}

fn json(arguments: &[&str]) -> Value {
    serde_json::from_slice(&success(arguments).stdout).unwrap()
}

/// Parse one canonical refusal envelope from the command's stderr.
fn refusal(arguments: &[&str]) -> Value {
    let result = output(arguments);
    assert!(!result.status.success());
    let text = String::from_utf8_lossy(&result.stderr);
    let text = text.trim().strip_prefix("Error: ").unwrap_or(text.trim());
    serde_json::from_str(text).unwrap()
}

#[test]
#[verifies("rule_porcelain_api_public_path", examples)]
#[verifies("rule_porcelain_api_uses_context", examples)]
fn api_get_reads_one_public_path_with_the_configured_context() {
    let (_directory, repo) = init();
    seed_source(&repo);

    let shared = json(&[
        "api",
        "sources/source_api",
        "--repo",
        &repo,
        "--format",
        "json",
    ]);
    let catalog = json(&[
        "sources",
        "get",
        "source_api",
        "--repo",
        &repo,
        "--format",
        "json",
    ]);
    assert_eq!(shared, catalog, "one public path gives one result");
    assert_eq!(shared["data"]["id"], "source_api");
    assert!(shared["meta"].is_object());

    let leading_slash = json(&[
        "api",
        "/sources/source_api",
        "--repo",
        &repo,
        "--format",
        "json",
    ]);
    assert_eq!(leading_slash, shared);
}

#[test]
fn api_get_defaults_to_get_and_prints_the_envelope() {
    let (_directory, repo) = init();
    seed_source(&repo);

    let plain = success(&["api", "sources/source_api", "--repo", &repo]);
    let printed: Value = serde_json::from_slice(&plain.stdout).unwrap();
    assert_eq!(printed["data"]["id"], "source_api");
}

#[test]
#[verifies("rule_porcelain_api_body_inputs", examples)]
fn api_post_creates_from_a_file_body_with_selected_method_and_headers() {
    let (_directory, repo) = init();
    let body = directory_file(
        &repo,
        "body.json",
        json!({
            "id": "source_file",
            "name": "Created from a file",
            "source_type": "document",
            "supersedes": []
        })
        .to_string()
        .as_str(),
    );

    success(&[
        "api",
        "sources",
        "--repo",
        &repo,
        "--method",
        "post",
        "--input",
        &body,
        "--header",
        "Idempotency-Key: cli-file-create",
    ]);

    let created = json(&["sources", "get", "source_file", "--repo", &repo]);
    assert_eq!(created["data"]["name"], "Created from a file");
}

#[test]
fn api_stdin_body_creates_from_standard_input() {
    let (_directory, repo) = init();

    provenance()
        .args([
            "api",
            "sources",
            "--repo",
            &repo,
            "--method",
            "post",
            "--input",
            "-",
            "--header",
            "Idempotency-Key: cli-stdin-create",
        ])
        .write_stdin(
            json!({
                "id": "source_stdin",
                "name": "Created from stdin",
                "source_type": "document",
                "supersedes": []
            })
            .to_string(),
        )
        .assert()
        .success();

    let created = json(&["sources", "get", "source_stdin", "--repo", &repo]);
    assert_eq!(created["data"]["name"], "Created from stdin");
}

#[test]
fn api_discovery_describes_the_live_catalog() {
    let (_directory, repo) = init();
    seed_source(&repo);

    let readable = String::from_utf8(success(&["api", "--repo", &repo]).stdout).unwrap();
    assert!(readable.starts_with("api routes: "), "{readable}");
    assert!(readable.contains("GET /requirements"), "{readable}");
    assert!(readable.contains("POST /sources"), "{readable}");

    let structured = json(&["api", "--repo", &repo, "--format", "json"]);
    let routes = structured["routes"].as_array().unwrap();
    let member = routes
        .iter()
        .find(|route| route["path"] == "/sources/{id}" && route["method"] == "get")
        .expect("the source member route is described");
    assert!(member["response_schema"].is_object());
    assert!(member["parameters"].as_array().unwrap().len() >= 1);
}

#[test]
fn api_unknown_paths_refuse_with_the_canonical_failure() {
    let (_directory, repo) = init();

    let printed = refusal(&["api", "nowhere/none", "--repo", &repo]);
    assert_eq!(printed["error"]["kind"], "unknown_operation");
}

#[test]
fn api_unsupported_methods_refuse_with_the_canonical_failure() {
    let (_directory, repo) = init();

    let printed = refusal(&[
        "api",
        "requirements/none/submit",
        "--repo",
        &repo,
        "--method",
        "get",
    ]);
    assert_eq!(printed["error"]["kind"], "method_not_allowed");
}

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

    let body = directory_file(&repo, "body.json", json!({"name": "X"}).to_string().as_str());
    provenance()
        .args([
            "api",
            "sources",
            "--repo",
            &repo,
            "--method",
            "get",
            "--input",
            &body,
        ])
        .assert()
        .failure();
}

fn directory_file(repo: &str, name: &str, content: &str) -> String {
    let path = std::path::Path::new(repo).join(name);
    std::fs::write(&path, content).unwrap();
    path.to_string_lossy().into_owned()
}
