use assert_cmd::Command;
use provenance_macros::verifies;

#[path = "export_fixture_support/mod.rs"]
mod export_fixture_support;

#[test]
#[verifies("rule_legacy_shard_frozen", examples)]
fn altered_replaced_or_omitted_shipped_disposition_audit_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let baseline = export_shipped(&dir);
    let audited = audited_proposal_ids(&baseline);
    for attack in ["altered", "replaced", "omitted"] {
        let mut value = baseline.clone();
        let dispositions = value["dispositions"].as_array_mut().unwrap();
        // Attack a row of the frozen shipped audit, not one of the repo's
        // modern tournament dispositions that share the array. The audit is
        // the set of rows that dispose of a terminal proposal, so ask that
        // question directly. A name pattern goes stale as the repository
        // gains modern rows.
        let target = dispositions
            .iter()
            .position(|row| audited.contains(row["proposal_id"].as_str().unwrap()))
            .unwrap();
        match attack {
            "altered" => {
                dispositions[target]["rationale"] = serde_json::json!(format!(
                    "{}x",
                    dispositions[target]["rationale"].as_str().unwrap()
                ));
            }
            "replaced" => {
                dispositions[target]["id"] = serde_json::json!("disposition_replacement");
            }
            "omitted" => {
                dispositions.remove(target);
            }
            _ => unreachable!(),
        }
        let input = dir.path().join(format!("{attack}.json"));
        std::fs::write(&input, serde_json::to_vec(&value).unwrap()).unwrap();
        let repo = dir.path().join(format!("repo-{attack}"));
        init(&repo);
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
            .failure()
            .stderr(predicates::str::contains(
                "frozen shipped-v1 disposition audit",
            ));
    }
}

#[test]
#[verifies("rule_legacy_shard_frozen", examples)]
fn exact_shipped_promotion_decisions_export_is_accepted() {
    let dir = tempfile::tempdir().unwrap();
    let mut legacy = export_shipped(&dir);
    legacy.as_object_mut().unwrap().remove("assertion_records");
    let dispositions = legacy
        .as_object_mut()
        .unwrap()
        .remove("dispositions")
        .unwrap();
    legacy["promotion_decisions"] = dispositions;
    let input = dir.path().join("legacy.json");
    std::fs::write(&input, serde_json::to_vec(&legacy).unwrap()).unwrap();
    let repo = dir.path().join("repo");
    init(&repo);
    seed_legacy_statements(&repo, &legacy);
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
        .success();
    assert!(repo
        .join(".provenance/state/scopes/default/ideation/dispositions.jsonl")
        .is_file());
    assert!(!repo
        .join(".provenance/state/scopes/default/ideation/promotion_decisions.jsonl")
        .exists());
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
        .success();
}

#[test]
#[verifies("rule_legacy_shard_frozen", examples)]
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

/// The proposal ids the frozen shipped-v1 disposition audit covers.
///
/// A disposition belongs to the audit when the proposal it disposes of is in a
/// terminal state. Modern tournament dispositions share the exported array and
/// point at proposals that are still proposed.
fn audited_proposal_ids(export: &serde_json::Value) -> std::collections::BTreeSet<String> {
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
    serde_json::from_slice(&std::fs::read(output).unwrap()).unwrap()
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
