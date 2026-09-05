use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use provenance_core::SUPPORTED_SCHEMA_VERSION;

#[test]
fn import_rejects_duplicate_thread_ids_and_names_the_offender() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    let export = prepare_export(&repo, dir.path());
    let mut value = read_export(&export);
    value["threads"] = serde_json::json!([
        thread("thread_duplicate", "resolved", 1),
        thread("thread_duplicate", "archived", 2)
    ]);
    write_export(&export, &value);

    import(&repo, &export)
        .failure()
        .stderr(predicates::str::contains(
            "duplicate thread id thread_duplicate",
        ));
}

#[test]
fn import_rejects_multiple_active_threads_and_names_the_parent_and_threads() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    let export = prepare_export(&repo, dir.path());
    let mut value = read_export(&export);
    value["threads"] = serde_json::json!([
        thread("thread_first", "active", 1),
        thread("thread_second", "active", 2),
        thread("thread_innocent", "resolved", 3)
    ]);
    write_export(&export, &value);

    import(&repo, &export).failure().stderr(
        predicates::str::contains("multiple active threads for requirement req_parent")
            .and(predicates::str::contains("thread_first"))
            .and(predicates::str::contains("thread_second"))
            .and(predicates::str::contains("thread_innocent").not()),
    );
}

#[test]
fn import_rejects_thread_in_unknown_scope_and_names_only_the_offender() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    let export = prepare_export(&repo, dir.path());
    let mut value = read_export(&export);
    value["threads"] = serde_json::json!([
        thread("thread_innocent", "resolved", 1),
        thread_in("thread_bad_scope", "missing", "req_parent", 2)
    ]);
    write_export(&export, &value);

    import(&repo, &export).failure().stderr(
        predicates::str::contains("thread thread_bad_scope is in unknown scope missing")
            .and(predicates::str::contains("thread_innocent").not()),
    );
}

#[test]
fn import_rejects_thread_with_unknown_parent_and_names_only_the_offender() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    let export = prepare_export(&repo, dir.path());
    let mut value = read_export(&export);
    value["threads"] = serde_json::json!([
        thread("thread_innocent", "resolved", 1),
        thread_in("thread_bad_parent", "default", "req_missing", 2)
    ]);
    write_export(&export, &value);

    import(&repo, &export).failure().stderr(
        predicates::str::contains(
            "thread thread_bad_parent has dangling reference: parent requirement req_missing",
        )
        .and(predicates::str::contains("thread_innocent").not()),
    );
}

#[test]
fn import_rejects_thread_whose_parent_exists_only_in_another_scope() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    let export = prepare_export(&repo, dir.path());
    add_scope_with_requirement(&repo, "other", "req_other");
    let mut value = read_export(&export);
    value["threads"] = serde_json::json!([
        thread("thread_innocent", "resolved", 1),
        thread_in("thread_cross_scope", "default", "req_other", 2)
    ]);
    write_export(&export, &value);

    import(&repo, &export).failure().stderr(
        predicates::str::contains(
            "thread thread_cross_scope has dangling reference: parent requirement req_other",
        )
        .and(predicates::str::contains("thread_innocent").not()),
    );
}

#[test]
fn import_accepts_threads_with_known_scopes_and_parents() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    let export = prepare_export(&repo, dir.path());
    let mut value = read_export(&export);
    value["threads"] = serde_json::json!([thread("thread_valid", "resolved", 1)]);
    write_export(&export, &value);

    import(&repo, &export).success();
}

fn prepare_export(repo: &std::path::Path, output_dir: &std::path::Path) -> std::path::PathBuf {
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "init",
            "--path",
            repo.to_str().unwrap(),
            "--scope",
            "default",
        ])
        .assert()
        .success();
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "requirements",
            "create",
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--id",
            "req_parent",
            "--statement",
            "Thread parent",
        ])
        .assert()
        .success();
    let export = output_dir.join("scope.json");
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "export",
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--format",
            "json",
            "--output",
            export.to_str().unwrap(),
        ])
        .assert()
        .success();
    export
}

fn thread(id: &str, status: &str, created_at: i64) -> serde_json::Value {
    let mut thread = thread_in(id, "default", "req_parent", created_at);
    thread["status"] = serde_json::json!(status);
    thread
}

fn thread_in(id: &str, scope_id: &str, parent_id: &str, created_at: i64) -> serde_json::Value {
    serde_json::json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
        "scope_id": scope_id,
        "id": id,
        "parent": {"node_type": "requirement", "node_id": parent_id},
        "status": "resolved",
        "created_at": created_at
    })
}

fn add_scope_with_requirement(repo: &std::path::Path, scope_id: &str, requirement_id: &str) {
    let manifest_path = repo.join(".provenance/state/manifest.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();
    manifest["scopes"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"id": scope_id, "path_prefix": scope_id}));
    std::fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    let requirements = repo.join(format!(
        ".provenance/state/scopes/{scope_id}/requirements/req.jsonl"
    ));
    std::fs::create_dir_all(requirements.parent().unwrap()).unwrap();
    std::fs::write(
        requirements,
        format!(
            "{}\n",
            serde_json::json!({
                "schema_version": SUPPORTED_SCHEMA_VERSION.0,
                "scope_id": scope_id,
                "id": requirement_id,
                "statement": "Parent in another scope",
                "status": "active"
            })
        ),
    )
    .unwrap();
}

fn read_export(path: &std::path::Path) -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn write_export(path: &std::path::Path, value: &serde_json::Value) {
    std::fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
}

fn import(repo: &std::path::Path, input: &std::path::Path) -> assert_cmd::assert::Assert {
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "import",
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--input",
            input.to_str().unwrap(),
        ])
        .assert()
}
