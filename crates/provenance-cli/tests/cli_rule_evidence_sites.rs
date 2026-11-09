//! The evidence answer of a Rule through the real CLI `api` route: the
//! `#[rule]` and `#[verifies]` attributes in the working tree are in the
//! answer, as `get --view impact` gives them, and the stamp names the file
//! scan as a live part.

use assert_cmd::Command;
use provenance_macros::verifies;
use serde_json::{json, Value};
use std::path::Path;

fn provenance() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
}

fn run(repo: &Path, args: &[&str]) {
    provenance().args(args).current_dir(repo).assert().success();
}

/// A new repository with one Rule, one `#[rule]` site and one
/// `#[verifies]` site, and no typed binding.
fn repo_with_attributes() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path();
    let path = repo.to_str().unwrap();
    run(
        repo,
        &[
            "init",
            "--path",
            path,
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ],
    );
    run(
        repo,
        &[
            "req_anchor",
            "create",
            "--type",
            "requirement",
            "--repo",
            path,
            "--scope",
            "default",
            "--statement",
            "The anchor requirement holds",
        ],
    );
    provenance()
        .args([
            "api", "rules", "--method", "post", "--input", "-", "--repo", path, "--scope",
            "default",
        ])
        .current_dir(repo)
        .write_stdin(
            json!({
                "id": "rule_served",
                "requirement_ids": ["req_anchor"],
                "resolution_ids": [],
                "statement": "The covered obligation holds",
                "status": "active",
                "severity": "high"
            })
            .to_string(),
        )
        .assert()
        .success();
    std::fs::create_dir_all(repo.join("src")).unwrap();
    std::fs::write(
        repo.join("src/serve.rs"),
        "#[rule(\"rule_served\")]\nfn serve() {}\n",
    )
    .unwrap();
    std::fs::create_dir_all(repo.join("tests")).unwrap();
    std::fs::write(
        repo.join("tests/serve.rs"),
        "#[verifies(\"rule_served\", examples)]\nfn serves() {}\n",
    )
    .unwrap();
    directory
}

fn evidence(repo: &Path) -> Value {
    let output = provenance()
        .args([
            "api",
            "rules/rule_served/evidence",
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            "default",
        ])
        .current_dir(repo)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
#[verifies("rule_evidence_lists_scanned_sites", examples)]
fn evidence_lists_the_attribute_sites_on_a_new_repository() {
    let repo = repo_with_attributes();

    let answer = evidence(repo.path());

    let data = &answer["data"];
    assert_eq!(
        data["implementation_sites"],
        json!([{"file": "src/serve.rs", "line": 1, "symbol": "serve"}]),
        "{answer:#}"
    );
    assert_eq!(
        data["verification_sites"],
        json!([{"method": "examples", "file": "tests/serve.rs", "line": 1, "symbol": "serves"}]),
        "{answer:#}"
    );
    assert_eq!(data["scan_cut"], json!(false));
    assert_eq!(data["implementation_bindings"], json!([]), "no typed binding");
}

#[test]
#[verifies("rule_stamp_lists_a_live_word_for_uncovered_parts", examples)]
fn the_evidence_stamp_names_the_file_scan_as_live() {
    let repo = repo_with_attributes();

    let answer = evidence(repo.path());

    let live = answer["meta"]["stamp"]["live"].as_array().unwrap();
    assert!(
        live.contains(&json!("scanned_sites")),
        "the attribute sites do not come from the projection: {live:?}"
    );
    let attested = answer["meta"]["stamp"]["attested"].as_array().unwrap();
    assert!(attested.contains(&json!("implementation_bindings")));
}
