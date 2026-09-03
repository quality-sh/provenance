use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;
use provenance_core::SUPPORTED_SCHEMA_VERSION;
use std::path::Path;

#[test]
fn check_rejects_thread_in_unknown_scope_and_names_only_the_offender() {
    let dir = tempfile::tempdir().unwrap();
    init(dir.path());
    let state = dir.path().join(".provenance/state");
    write_jsonl(
        &state.join("scopes/default/requirements/req.jsonl"),
        &requirement("req_parent"),
    );
    write_jsonl(
        &state.join("scopes/default/threads/threads.jsonl"),
        &format!(
            "{}\n{}",
            thread("thread_innocent", "default", "req_parent", 1),
            thread("thread_bad_scope", "missing", "req_parent", 2)
        ),
    );

    check(dir.path()).failure().stderr(
        contains("thread thread_bad_scope is in unknown scope missing")
            .and(contains("thread_innocent").not()),
    );
}

#[test]
fn check_rejects_thread_with_unknown_parent_and_names_only_the_offender() {
    let dir = tempfile::tempdir().unwrap();
    init(dir.path());
    let state = dir.path().join(".provenance/state");
    write_jsonl(
        &state.join("scopes/default/requirements/req.jsonl"),
        &requirement("req_parent"),
    );
    write_jsonl(
        &state.join("scopes/default/threads/threads.jsonl"),
        &format!(
            "{}\n{}",
            thread("thread_innocent", "default", "req_parent", 1),
            thread("thread_bad_parent", "default", "req_missing", 2)
        ),
    );

    check(dir.path()).failure().stderr(
        contains("thread thread_bad_parent has dangling reference: parent requirement req_missing")
            .and(contains("thread_innocent").not()),
    );
}

#[test]
fn check_rejects_thread_whose_parent_exists_only_in_another_scope() {
    let dir = tempfile::tempdir().unwrap();
    init(dir.path());
    let state = dir.path().join(".provenance/state");
    add_scope(dir.path(), "other", "other");
    write_jsonl(
        &state.join("scopes/default/requirements/req.jsonl"),
        &requirement("req_parent"),
    );
    write_jsonl(
        &state.join("scopes/other/requirements/req.jsonl"),
        &requirement_in("other", "req_other"),
    );
    write_jsonl(
        &state.join("scopes/default/threads/threads.jsonl"),
        &format!(
            "{}\n{}",
            thread("thread_innocent", "default", "req_parent", 1),
            thread("thread_cross_scope", "default", "req_other", 2)
        ),
    );

    check(dir.path()).failure().stderr(
        contains("thread thread_cross_scope has dangling reference: parent requirement req_other")
            .and(contains("thread_innocent").not()),
    );
}

#[test]
fn check_accepts_threads_with_known_scopes_and_parents() {
    let dir = tempfile::tempdir().unwrap();
    init(dir.path());
    let state = dir.path().join(".provenance/state");
    write_jsonl(
        &state.join("scopes/default/requirements/req.jsonl"),
        &requirement("req_parent"),
    );
    write_jsonl(
        &state.join("scopes/default/threads/threads.jsonl"),
        &thread("thread_valid", "default", "req_parent", 1),
    );

    check(dir.path()).success();
}

fn requirement(id: &str) -> String {
    requirement_in("default", id)
}

fn requirement_in(scope_id: &str, id: &str) -> String {
    format!(
        r#"{{"schema_version":{version},"scope_id":"{scope_id}","id":"{id}","statement":"Parent","status":"active"}}"#,
        version = SUPPORTED_SCHEMA_VERSION.0
    )
}

fn add_scope(repo: &Path, scope_id: &str, path_prefix: &str) {
    let path = repo.join(".provenance/state/manifest.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    manifest["scopes"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"id": scope_id, "path_prefix": path_prefix}));
    std::fs::write(path, serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();
}

fn thread(id: &str, scope_id: &str, parent_id: &str, created_at: i64) -> String {
    format!(
        r#"{{"schema_version":{version},"scope_id":"{scope_id}","id":"{id}","parent":{{"node_type":"requirement","node_id":"{parent_id}"}},"status":"resolved","created_at":{created_at}}}"#,
        version = SUPPORTED_SCHEMA_VERSION.0
    )
}

fn init(repo: &Path) {
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "init",
            "--path",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();
}

fn check(repo: &Path) -> assert_cmd::assert::Assert {
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "check",
            "--repo",
            repo.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
}

fn write_jsonl(path: &Path, records: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, format!("{records}\n")).unwrap();
}
