use assert_cmd::Command;

#[path = "export_fixture_support/mod.rs"]
mod export_fixture_support;

// Implementation aid: rule_legacy_terminal_proposals_frozen is archived. The import check stays until provenance-u9mg removes it.
#[test]
fn a_disposition_on_a_shipped_terminal_proposal_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let mut value = export_shipped(&dir);
    let terminal = terminal_proposal_ids(&value).into_iter().next().unwrap();
    let dispositions = value["dispositions"].as_array_mut().unwrap();
    let mut forged = dispositions[0].clone();
    forged["id"] = serde_json::json!("disposition_on_terminal");
    forged["proposal_id"] = serde_json::json!(terminal);
    dispositions.push(forged);
    let input = dir.path().join("forged.json");
    std::fs::write(&input, serde_json::to_vec(&value).unwrap()).unwrap();
    let repo = dir.path().join("repo");
    init(&repo);
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "import",
            "--format",
            "json",
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--input",
            input.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "a terminal proposal row takes no disposition",
        ));
}

// Implementation aid: rule_legacy_terminal_proposals_frozen is archived. The import check stays until provenance-u9mg removes it.
#[test]
fn import_cannot_omit_entire_existing_shipped_legacy_terminal_set() {
    let dir = tempfile::tempdir().unwrap();
    let mut shipped = export_shipped(&dir);
    let repo = dir.path().join("repo");
    init(&repo);
    seed_legacy_statements(&repo, &shipped);
    let complete = dir.path().join("complete.json");
    std::fs::write(&complete, serde_json::to_vec(&shipped).unwrap()).unwrap();
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "import",
            "--format",
            "json",
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--input",
            complete.to_str().unwrap(),
        ])
        .assert()
        .success();

    shipped["proposal_cards"]
        .as_array_mut()
        .unwrap()
        .retain(|proposal| proposal["promotion_state"].as_str() == Some("proposed"));
    shipped["dispositions"] = serde_json::json!([]);
    let omitted = dir.path().join("omitted-all.json");
    std::fs::write(&omitted, serde_json::to_vec(&shipped).unwrap()).unwrap();
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "import",
            "--format",
            "json",
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--input",
            omitted.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains("immutable proposal"));
}

/// The ids of the shipped proposals that are in a terminal state.
fn terminal_proposal_ids(export: &serde_json::Value) -> std::collections::BTreeSet<String> {
    export["proposal_cards"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|proposal| proposal["promotion_state"].as_str() != Some("proposed"))
        .map(|proposal| proposal["id"].as_str().unwrap().to_string())
        .collect()
}

fn export_shipped(dir: &tempfile::TempDir) -> serde_json::Value {
    let output = dir.path().join("shipped.json");
    let shipped = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let portable = dir.path().join("portable-shipped");
    export_fixture_support::copy_portable_state(shipped, &portable);
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "export",
            "--repo",
            portable.to_str().unwrap(),
            "--scope",
            "default",
            "--format",
            "json",
            "--output",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
    let mut document = serde_json::from_slice(&std::fs::read(output).unwrap()).unwrap();
    export_fixture_support::keep_decision_links_pending(&mut document);
    document
}

fn init(repo: &std::path::Path) {
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "init",
            "--path",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--disposition-actor-id",
            "codex-review-panel-gpt55-medium",
            "--disposition-actor-id",
            "ben_nasraoui",
        ])
        .assert()
        .success();
}

fn seed_legacy_statements(repo: &std::path::Path, export: &serde_json::Value) {
    for (family, shard) in [("requirements", "req.jsonl"), ("rules", "rule.jsonl")] {
        let directory = repo.join(format!(".provenance/state/scopes/default/{family}"));
        std::fs::create_dir_all(&directory).unwrap();
        let mut bytes = Vec::new();
        for record in export[family].as_array().unwrap() {
            serde_json::to_writer(&mut bytes, record).unwrap();
            bytes.push(b'\n');
        }
        std::fs::write(directory.join(shard), bytes).unwrap();
    }
}
