use super::*;
use provenance_core::SUPPORTED_SCHEMA_VERSION;
use provenance_macros::verifies;

/// A record at an unreadable version never reaches the projection.
///
/// The pinned graph keeps its own per-record version check for a graph that
/// arrives as a document, but a record read off disk is refused earlier, by
/// the store's read guard, which is why the message names the file and line
/// rather than the family. Issuing a reference over such a record is what this
/// pins: no reference is minted for a record this build cannot read.
#[test]
fn issue_rejects_unsupported_pinned_record_schema_versions() {
    let temp = committed_store();
    provenance(temp.path())
        .args([
            "sources",
            "create",
            "--repo",
            ".",
            "--scope",
            "default",
            "--id",
            "source_v2",
            "--name",
            "Future source",
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
            &format!("\"schema_version\":{}", SUPPORTED_SCHEMA_VERSION.0),
            &format!("\"schema_version\":{}", SUPPORTED_SCHEMA_VERSION.0 + 1),
        ),
    )
    .unwrap();
    git(temp.path(), &["add", ".provenance/state"]);
    git(temp.path(), &["commit", "-qm", "add unsupported source"]);

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
        .stderr(predicate::str::contains("sources/source.jsonl line 1"))
        .stderr(predicate::str::contains(format!(
            "record source_v2 has schema_version {}, but this build reads schema_version {} only",
            SUPPORTED_SCHEMA_VERSION.0 + 1,
            SUPPORTED_SCHEMA_VERSION.0
        )));
}

#[test]
fn issue_rejects_unknown_fields_in_pinned_graph_records() {
    let temp = committed_store();
    provenance(temp.path())
        .args([
            "sources",
            "create",
            "--repo",
            ".",
            "--scope",
            "default",
            "--id",
            "source_typo",
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
        serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0 + 1, "scope_id":"future","id":"req_future","statement":"Future","status":"active","future_field":true}).to_string() + "\n",
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
