use assert_cmd::Command;
use predicates::str::contains;

const EDGE_ID: &str = "refinesinto_requirement_req_parent_to_requirement_req_child";

#[test]
fn cli_edges_create_list_delete_and_validate_endpoints() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo").to_string_lossy().to_string();

    init(&repo);
    create_requirement(&repo, "req_parent", "Parent requirement");
    create_requirement(&repo, "req_child", "Child requirement");

    create_refines_edge(&repo);
    list_edges_includes_created_edge(&repo);
    rejects_invalid_endpoint_shape(&repo);
    rejects_missing_endpoint_record(&repo);
    delete_created_edge(&repo);
    list_edges_is_empty(&repo);
}

fn create_refines_edge(repo: &str) {
    provenance(&[
        "edges",
        "create",
        "--repo",
        repo,
        "--scope",
        "default",
        "--type",
        "refines_into",
        "--from-type",
        "requirement",
        "--from-id",
        "req_parent",
        "--to-type",
        "requirement",
        "--to-id",
        "req_child",
        "--format",
        "json",
    ])
    .success()
    .stdout(contains(format!(r#""id": "{EDGE_ID}""#)))
    .stdout(contains(r#""edge_type": "refines_into""#))
    .stdout(contains(r#""from_id": "req_parent""#))
    .stdout(contains(r#""to_id": "req_child""#));
}

fn list_edges_includes_created_edge(repo: &str) {
    provenance(&[
        "edges", "list", "--repo", repo, "--scope", "default", "--format", "json",
    ])
    .success()
    .stdout(contains(EDGE_ID));
}

fn rejects_invalid_endpoint_shape(repo: &str) {
    provenance(&[
        "edges",
        "create",
        "--repo",
        repo,
        "--scope",
        "default",
        "--type",
        "references",
        "--from-type",
        "requirement",
        "--from-id",
        "req_parent",
        "--to-type",
        "source",
        "--to-id",
        "source_policy",
    ])
    .failure()
    .stderr(contains("invalid References edge endpoint"));
}

fn rejects_missing_endpoint_record(repo: &str) {
    provenance(&[
        "edges",
        "create",
        "--repo",
        repo,
        "--scope",
        "default",
        "--type",
        "refines_into",
        "--from-type",
        "requirement",
        "--from-id",
        "req_parent",
        "--to-type",
        "requirement",
        "--to-id",
        "req_missing",
    ])
    .failure()
    .stderr(contains("to endpoint does not exist"));
}

fn delete_created_edge(repo: &str) {
    provenance(&[
        "edges", "delete", "--repo", repo, "--scope", "default", "--id", EDGE_ID, "--format",
        "json",
    ])
    .success()
    .stdout(contains(r#""edge_type": "refines_into""#));
}

fn list_edges_is_empty(repo: &str) {
    provenance(&[
        "edges", "list", "--repo", repo, "--scope", "default", "--format", "json",
    ])
    .success()
    .stdout(contains("[]"));
}

fn init(repo: &str) {
    provenance(&[
        "init",
        "--path",
        repo,
        "--scope",
        "default",
        "--path-prefix",
        ".",
    ])
    .success();
}

fn create_requirement(repo: &str, id: &str, statement: &str) {
    provenance(&[
        "requirements",
        "create",
        "--repo",
        repo,
        "--scope",
        "default",
        "--id",
        id,
        "--statement",
        statement,
        "--format",
        "json",
    ])
    .success();
}

fn provenance(args: &[&str]) -> assert_cmd::assert::Assert {
    Command::cargo_bin("provenance")
        .unwrap()
        .args(args)
        .assert()
}
