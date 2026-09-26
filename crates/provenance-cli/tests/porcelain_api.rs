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
    envelope(&String::from_utf8_lossy(&result.stderr))
}

fn envelope(stderr: &str) -> Value {
    let text = stderr.trim();
    let text = text.strip_prefix("Error: ").unwrap_or(text);
    serde_json::from_str(text).unwrap_or_else(|error| panic!("{error}: {stderr}"))
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
    let target_first = json(&["source_api", "get", "--repo", &repo, "--format", "json"]);
    assert_eq!(shared["data"]["id"], "source_api");
    assert_eq!(
        shared["data"]["name"], target_first["record"]["value"]["name"],
        "one public path reads one record"
    );
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

    let upper = json(&[
        "api",
        "sources/source_api",
        "--repo",
        &repo,
        "--method",
        "GET",
    ]);
    assert_eq!(upper, printed, "the method name takes any letter case");
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

    let created = json(&["source_file", "get", "--repo", &repo, "--format", "json"]);
    assert_eq!(created["record"]["value"]["name"], "Created from a file");
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

    let created = json(&["source_stdin", "get", "--repo", &repo, "--format", "json"]);
    assert_eq!(created["record"]["value"]["name"], "Created from stdin");
}

#[test]
#[verifies("rule_porcelain_api_catalog_discovery", examples)]
fn api_discovery_describes_the_live_catalog() {
    let (_directory, repo) = init();
    seed_source(&repo);

    let readable = String::from_utf8(success(&["api", "--repo", &repo]).stdout).unwrap();
    assert!(readable.starts_with("api routes: "), "{readable}");
    assert!(readable.contains("GET /requirements"), "{readable}");
    assert!(readable.contains("POST /sources"), "{readable}");
    assert!(readable.contains("- GET /sources/{id}\n"), "{readable}");
    assert!(
        readable.contains("  inputs: id (path, required)\n"),
        "{readable}"
    );
    assert!(
        readable.contains("Idempotency-Key (header, required)"),
        "{readable}"
    );
    assert!(
        readable.contains("  inputs with query=neighbors: "),
        "{readable}"
    );
    assert!(
        readable
            .trim_end()
            .ends_with("Use --format json for the full request and response schemas."),
        "{readable}"
    );

    let structured = json(&["api", "--repo", &repo, "--format", "json"]);
    let routes = structured["routes"].as_array().unwrap();
    let member = routes
        .iter()
        .find(|route| route["path"] == "/sources/{id}" && route["method"] == "get")
        .expect("the source member route is described");
    let forms = member["queries"].as_array().unwrap();
    assert!(!forms.is_empty());
    let base = forms
        .iter()
        .find(|form| form["query"].is_null())
        .expect("the base form is described");
    assert!(!base["success_schema"].as_object().unwrap().is_empty());
    assert!(!base["parameters"].as_array().unwrap().is_empty());
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
#[verifies("rule_porcelain_api_public_path", examples)]
fn api_query_values_follow_their_contracts() {
    let (_directory, repo) = init();
    provenance()
        .args([
            "sources",
            "create",
            "--repo",
            &repo,
            "--id",
            "source_q",
            "--name",
            "Needle source",
        ])
        .assert()
        .success();
    provenance()
        .args([
            "requirements",
            "create",
            "--repo",
            &repo,
            "--id",
            "req_q",
            "--statement",
            "Shared needle requirement.",
        ])
        .assert()
        .success();
    provenance()
        .args([
            "rules",
            "create",
            "--repo",
            &repo,
            "--id",
            "rule_q",
            "--requirement-id",
            "req_q",
            "--statement",
            "Shared needle rule.",
            "--severity",
            "medium",
        ])
        .assert()
        .success();

    let base = json(&["api", "rules", "--repo", &repo, "--query", "limit=1"]);
    assert_eq!(base["data"]["items"].as_array().unwrap().len(), 1);

    let searched = json(&[
        "api",
        "rules",
        "--repo",
        &repo,
        "--query",
        "query=search",
        "--query",
        "text=needle",
    ]);
    let items = searched["data"]["items"].as_array().unwrap();
    assert!(items.iter().any(|item| item["id"] == "rule_q"));

    let neighbors = json(&[
        "api",
        "requirements/req_q",
        "--repo",
        &repo,
        "--query",
        "query=neighbors",
    ]);
    assert!(neighbors["data"].is_object(), "{neighbors}");

    let unknown_query = refusal(&[
        "api",
        "requirements/req_q",
        "--repo",
        &repo,
        "--query",
        "query=stale",
    ]);
    assert_eq!(unknown_query["error"]["kind"], "invalid_input");
    assert_eq!(unknown_query["error"]["field"], "query");
}

fn directory_file(repo: &str, name: &str, content: &str) -> String {
    let path = std::path::Path::new(repo).join(name);
    std::fs::write(&path, content).unwrap();
    path.to_string_lossy().into_owned()
}

#[path = "porcelain_api/options.rs"]
mod options;
#[path = "porcelain_api/scope.rs"]
mod scope;
