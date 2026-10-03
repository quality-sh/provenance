use super::support::{
    export_scope, import_scope, init_repo, init_repo_with_actors, provenance, shipped_repo,
    write_json,
};

#[test]
fn shipped_legacy_export_imports_when_legacy_statements_are_already_canonical() {
    let dir = tempfile::tempdir().unwrap();
    let fresh = dir.path().join("fresh");
    let shipped = dir.path().join("shipped");
    let export = dir.path().join("shipped.json");
    crate::export_fixture_support::copy_portable_state(shipped_repo(), &shipped);
    export_scope(&shipped, &export).success();
    init_repo_with_actors(&fresh, &["codex-review-panel-gpt55-medium", "ben_nasraoui"]);
    seed_statement_shards(&shipped, &fresh);
    import_scope(&fresh, &export).success();
    for command in ["check", "materialize"] {
        run_repo_command(command, &fresh);
    }
}

fn seed_statement_shards(source: &std::path::Path, destination: &std::path::Path) {
    for family in ["requirements", "rules"] {
        copy_tree(
            &source.join(format!(".provenance/state/scopes/default/{family}")),
            &destination.join(format!(".provenance/state/scopes/default/{family}")),
        );
    }
}

#[test]
fn historical_shipped_manifest_without_actor_allowlist_remains_readable() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("historical");
    let shipped_state = shipped_repo().join(".provenance/state");
    copy_tree(&shipped_state, &repo.join(".provenance/state"));
    let manifest_path = repo.join(".provenance/state/manifest.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();
    manifest
        .as_object_mut()
        .unwrap()
        .remove("disposition_actor_ids");
    write_json(&manifest_path, &manifest);
    // A historical repo predates the modern disposition lifecycle; the live
    // repo's modern dispositions do not belong in the simulated history.
    let modern_dispositions =
        repo.join(".provenance/state/scopes/default/ideation/dispositions.jsonl");
    if modern_dispositions.exists() {
        std::fs::remove_file(&modern_dispositions).unwrap();
    }
    crate::export_fixture_support::make_default_scope_portable(&repo);

    for command in ["check", "materialize"] {
        run_repo_command(command, &repo);
    }
    provenance()
        .args([
            "export",
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--format",
            "json",
        ])
        .assert()
        .success();
}

#[test]
fn one_byte_change_to_shipped_legacy_terminal_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let fresh = dir.path().join("fresh");
    let shipped = dir.path().join("shipped");
    let export = dir.path().join("forged-shipped.json");
    crate::export_fixture_support::copy_portable_state(shipped_repo(), &shipped);
    export_scope(&shipped, &export).success();
    let mut value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&export).unwrap()).unwrap();
    let terminal = value["proposal_cards"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|proposal| proposal["promotion_state"] != "proposed")
        .unwrap();
    terminal["summary"] = serde_json::json!(format!("{}x", terminal["summary"].as_str().unwrap()));
    write_json(&export, &value);
    init_repo(&fresh, None);
    import_scope(&fresh, &export)
        .failure()
        .stderr(predicates::str::contains("frozen shipped-v1 fingerprint"));
}

fn run_repo_command(command: &str, repo: &std::path::Path) {
    provenance()
        .args([
            command,
            "--repo",
            repo.to_str().unwrap(),
            "--format",
            "json",
        ])
        .assert()
        .success();
}

fn copy_tree(source: &std::path::Path, destination: &std::path::Path) {
    std::fs::create_dir_all(destination).unwrap();
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let target = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}
