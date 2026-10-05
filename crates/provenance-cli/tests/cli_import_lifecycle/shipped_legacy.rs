use super::support::{
    export_scope, import_scope, init_repo, init_repo_with_actors, provenance, shipped_repo,
    write_json,
};
use provenance_macros::verifies;

#[test]
#[verifies("rule_legacy_shard_frozen", examples)]
/// This test covers the export, import, validation, and materialization flow.
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
    let imported = dir.path().join("imported.json");
    export_scope(&fresh, &imported).success();
    let original: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&export).unwrap()).unwrap();
    let imported: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&imported).unwrap()).unwrap();
    assert_eq!(imported["proposal_cards"], original["proposal_cards"]);
    assert_eq!(imported["dispositions"], original["dispositions"]);
    for command in ["check", "materialize"] {
        run_repo_command(command, &fresh);
    }
}

fn seed_statement_shards(source: &std::path::Path, destination: &std::path::Path) {
    for family in ["requirements", "rules"] {
        crate::export_fixture_support::copy_tree(
            &source.join(format!(".provenance/state/scopes/default/{family}")),
            &destination.join(format!(".provenance/state/scopes/default/{family}")),
        );
    }
}

#[test]
#[verifies("rule_legacy_terminal_proposals_frozen", examples)]
/// This test covers the export, modification, and rejected import flow.
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
