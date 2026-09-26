use super::{directory_file, init, json, refusal, success};
use provenance_macros::verifies;

fn add_scope(repo: &str, scope: &str) {
    let path = std::path::Path::new(repo).join(".provenance/state/manifest.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    manifest["scopes"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"id": scope, "path_prefix": scope}));
    std::fs::write(path, serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();
}

#[test]
#[verifies("rule_porcelain_api_uses_context", examples)]
fn api_reads_and_writes_in_the_selected_scope() {
    let (_directory, repo) = init();
    add_scope(&repo, "other");
    let body = directory_file(
        &repo,
        "requirement.json",
        serde_json::json!({
            "id": "req_scoped",
            "statement": "The selected scope holds this requirement.",
            "actor": "api",
            "status": "active",
            "depends_on": [],
            "supersedes": []
        })
        .to_string()
        .as_str(),
    );

    success(&[
        "api",
        "requirements",
        "--repo",
        &repo,
        "--scope",
        "other",
        "--method",
        "post",
        "--input",
        &body,
        "--header",
        "Idempotency-Key: cli-scoped-create",
    ]);

    let scoped = json(&[
        "api",
        "requirements/req_scoped",
        "--repo",
        &repo,
        "--scope",
        "other",
    ]);
    assert_eq!(scoped["data"]["id"], "req_scoped");
    let target_first = json(&[
        "req_scoped",
        "get",
        "--repo",
        &repo,
        "--scope",
        "other",
        "--format",
        "json",
    ]);
    assert_eq!(
        target_first["record"]["value"]["statement"],
        scoped["data"]["statement"]
    );

    let default_scope = refusal(&["api", "requirements/req_scoped", "--repo", &repo]);
    assert_eq!(default_scope["error"]["kind"], "resource_not_found");
}
