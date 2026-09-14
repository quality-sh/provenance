use assert_cmd::Command;
use provenance_macros::verifies;
use serde_json::{json, Value};

fn provenance() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
}

fn invoke(repo: &str, operation: &str, request: &Value) -> Value {
    let output = provenance()
        .args([
            "sdk", operation, "--repo", repo, "--scope", "default", "--format", "json",
        ])
        .write_stdin(serde_json::to_vec(request).unwrap())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "sdk {operation} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
#[verifies("rule_review_writes_use_rust_operations", examples)]
fn cli_exposes_requirement_review_writes_reads_and_receipts() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap();
    provenance()
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

    let creation = json!({
        "request_id": "create_request",
        "actor": "worker",
        "create": {
            "scope_id": "default",
            "id": "req_reviewed",
            "statement": "The review page saves Requirement changes.",
            "description": null,
            "status": "discovery",
            "domain_id": null,
            "refines": null,
            "depends_on": [],
            "supersedes": [],
            "spawned_by": null,
            "origin_thread": null,
            "origin_message": null
        },
        "origin": null
    });
    let created = invoke(repo, "create-review-requirement", &creation);
    assert_eq!(created["outcome"], "created");
    assert_eq!(
        invoke(repo, "requirement-creation-receipt", &creation),
        created
    );

    let edit_state = invoke(
        repo,
        "requirement-edit-state",
        &json!({"requirement_id":"req_reviewed"}),
    );
    let save = json!({
        "request_id": "save_request",
        "actor": "worker",
        "expected_etag": edit_state["etag"],
        "update": {
            "scope_id": "default",
            "id": "req_reviewed",
            "description": "The local host keeps the durable result."
        },
        "relationships": null
    });
    let saved = invoke(repo, "save-requirement", &save);
    let receipt = json!({
        "requirement_id": "req_reviewed",
        "request_id": "save_request",
        "actor": "worker",
        "declared_by": null
    });
    assert_eq!(invoke(repo, "requirement-save-receipt", &receipt), saved);

    let history = invoke(
        repo,
        "review-history",
        &json!({"requirement_id":"req_reviewed","limit":1,"cursor":null}),
    );
    assert_eq!(history["operation"], "review-history");
    assert_eq!(history["entries"].as_array().unwrap().len(), 1);
    assert!(history["next_cursor"].is_string());
}
