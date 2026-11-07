use super::*;
use provenance_macros::verifies;

#[test]
fn issue_rejects_unknown_fields_in_pinned_graph_records() {
    let temp = committed_store();
    provenance(temp.path())
        .args([
            "source_typo",
            "create",
            "--type",
            "source",
            "--repo",
            ".",
            "--scope",
            "default",
            "--name",
            "Typo source",
        ])
        .assert()
        .success();
    let source_path = temp
        .path()
        .join(".provenance/state/scopes/default/sources/source.jsonl");
    let source = std::fs::read_to_string(&source_path).unwrap();
    std::fs::write(
        &source_path,
        source.replace(
            "\"name\":\"Typo source\"",
            "\"name\":\"Typo source\",\"naem\":\"lost\"",
        ),
    )
    .unwrap();
    git(temp.path(), &["add", ".provenance/state"]);
    git(temp.path(), &["commit", "-qm", "add malformed source"]);

    provenance(temp.path())
        .args([
            "graph-reference",
            "issue",
            "--repo",
            ".",
            "--scope",
            "default",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown field"));
}

#[test]
#[verifies("rule_pinned_scope_ownership", examples)]
fn selected_scope_ignores_future_data_from_another_scope() {
    let temp = committed_store();
    let manifest_path = temp.path().join(".provenance/state/manifest.json");
    let mut manifest: Value =
        serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();
    manifest["scopes"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "id": "future",
            "path_prefix": "future",
            "future_field": true
        }));
    std::fs::write(&manifest_path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    let future_requirements = temp
        .path()
        .join(".provenance/state/scopes/future/requirements/req.jsonl");
    std::fs::create_dir_all(future_requirements.parent().unwrap()).unwrap();
    std::fs::write(
        future_requirements,
        serde_json::json!({"scope_id":"future","id":"req_future","statement":"Future","status":"active","future_field":true}).to_string() + "\n",
    )
    .unwrap();
    git(temp.path(), &["add", ".provenance/state"]);
    git(temp.path(), &["commit", "-qm", "add future scope data"]);

    let reference = issue(temp.path(), &[]);
    let reference_path = write_reference(temp.path(), &reference);
    provenance(temp.path())
        .args([
            "graph-reference",
            "exact-export",
            "--repo",
            ".",
            "--reference",
            reference_path.to_str().unwrap(),
        ])
        .assert()
        .success();
}
