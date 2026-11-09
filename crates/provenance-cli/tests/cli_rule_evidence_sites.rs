//! The evidence answer of a Rule through the real CLI `api` route: the
//! `#[rule]` and `#[verifies]` attributes in the working tree are in the
//! answer, as `get --view impact` gives them, and the stamp names the file
//! scan as a live part.

use provenance_macros::verifies;
use serde_json::{json, Value};
use std::path::Path;

#[path = "support/review_paths.rs"]
mod review_paths;
#[path = "support/stale_repo.rs"]
mod stale_repo;
use review_paths::provenance;
use stale_repo::{apply_spec, init_repo, write};

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
    write(
        repo,
        "src/serve.rs",
        "#[rule(\"rule_served\")]\nfn serve() {}\n",
    );
    write(
        repo,
        "tests/serve.rs",
        "#[verifies(\"rule_served\", examples)]\nfn serves() {}\n",
    );
    directory
}

/// A git repository whose Rule `rule_start` has a typed implementation
/// binding from a spec, one `#[rule]` site and one `#[verifies]` site.
fn repo_with_a_typed_binding_and_attributes() -> tempfile::TempDir {
    let directory = init_repo();
    let repo = directory.path();
    write(
        repo,
        "src/runtime.ts",
        "export function startWorkflow() { return 'started'; }\n",
    );
    apply_spec(
        repo,
        &json!({
            "spec": "runtime",
            "declared_by": "spec://typescript/runtime",
            "requirements": [{
                "key": "workflows",
                "statement": "Accepted workflows execute"
            }],
            "rules": [{
                "key": "start",
                "id": "rule_start",
                "requirement": "workflows",
                "statement": "Accepted workflows start",
                "implementation": {
                    "file": "src/runtime.ts",
                    "symbol": "startWorkflow"
                }
            }]
        }),
    );
    write(
        repo,
        "src/start.rs",
        "#[rule(\"rule_start\")]\nfn start() {}\n",
    );
    write(
        repo,
        "tests/start.rs",
        "#[verifies(\"rule_start\", examples)]\nfn starts() {}\n",
    );
    directory
}

fn evidence(repo: &Path, rule: &str) -> Value {
    let output = provenance()
        .args([
            "api",
            &format!("rules/{rule}/evidence"),
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

/// The impact answer that `get --view impact` gives for `rule`.
fn impact(repo: &Path, rule: &str) -> Value {
    let output = provenance()
        .args([
            rule,
            "get",
            "--view",
            "impact",
            "--repo",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--format",
            "json",
        ])
        .current_dir(repo)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let answer: Value = serde_json::from_slice(&output.stdout).unwrap();
    find_impact(&answer)
        .unwrap_or_else(|| panic!("no impact answer in {answer:#}"))
        .clone()
}

fn find_impact(value: &Value) -> Option<&Value> {
    match value {
        Value::Object(map) if map.contains_key("affected_rules") => Some(value),
        Value::Object(map) => map.values().find_map(find_impact),
        Value::Array(items) => items.iter().find_map(find_impact),
        _ => None,
    }
}

fn affected_rule<'a>(impact: &'a Value, rule: &str) -> &'a Value {
    impact["affected_rules"]
        .as_array()
        .unwrap()
        .iter()
        .find(|affected| affected["id"] == rule)
        .unwrap_or_else(|| panic!("{rule} is not in {impact:#}"))
}

#[test]
#[verifies("rule_evidence_gives_scanned_sites", examples)]
fn evidence_lists_the_attribute_sites_on_a_new_repository() {
    let repo = repo_with_attributes();

    let answer = evidence(repo.path(), "rule_served");

    let data = &answer["data"];
    assert_eq!(
        data["implementations"],
        json!([{"file": "src/serve.rs", "line": 1, "symbol": "serve"}]),
        "{answer:#}"
    );
    assert_eq!(
        data["verifications"],
        json!([{"method": "examples", "file": "tests/serve.rs", "line": 1, "symbol": "serves"}]),
        "{answer:#}"
    );
    assert_eq!(data["scan_cut"], json!(false));
    assert_eq!(
        data["implementation_bindings"],
        json!([]),
        "no typed binding"
    );
}

/// The impact answer and the evidence answer name the same data with the
/// same field names, so a reader of either finds the `#[rule]` site and
/// the typed binding in one list.
#[test]
#[verifies("rule_evidence_gives_scanned_sites", examples)]
fn evidence_gives_the_impact_sites_of_a_rule_with_a_typed_binding() {
    let repo = repo_with_a_typed_binding_and_attributes();

    let answer = evidence(repo.path(), "rule_start");
    let impact = impact(repo.path(), "rule_start");

    let data = &answer["data"];
    let affected = affected_rule(&impact, "rule_start");
    assert_eq!(
        data["implementations"], affected["implementations"],
        "{answer:#}\n{impact:#}"
    );
    assert_eq!(
        data["verifications"], affected["verifications"],
        "{answer:#}\n{impact:#}"
    );
    assert_eq!(
        data["implementations"],
        json!([
            {"file": "src/runtime.ts", "symbol": "startWorkflow"},
            {"file": "src/start.rs", "line": 1, "symbol": "start"}
        ]),
        "{answer:#}"
    );
    assert_eq!(data["implementation_bindings"].as_array().unwrap().len(), 1);
    assert_eq!(data["scan_cut"], impact["scan_cut"]);
}

/// A scan limit of one file cuts the scan of a tree with more files. Both
/// answers say so and give the same lower bound.
#[test]
#[verifies("rule_evidence_gives_scanned_sites", examples)]
fn evidence_says_when_the_scan_was_cut_as_impact_does() {
    let repo = repo_with_a_typed_binding_and_attributes();
    for index in 0..4 {
        write(
            repo.path(),
            &format!("src/extra_{index}.rs"),
            format!("#[rule(\"rule_start\")]\nfn extra_{index}() {{}}\n"),
        );
    }
    write(
        repo.path(),
        ".provenance/settings.json",
        r#"{"read": {"scan_limit": 1}}"#,
    );

    let answer = evidence(repo.path(), "rule_start");
    let impact = impact(repo.path(), "rule_start");

    let data = &answer["data"];
    assert_eq!(data["scan_cut"], json!(true), "{answer:#}");
    assert_eq!(impact["scan_cut"], json!(true), "{impact:#}");
    let affected = affected_rule(&impact, "rule_start");
    assert_eq!(data["implementations"], affected["implementations"]);
    assert_eq!(data["verifications"], affected["verifications"]);
    assert!(
        data["implementations"].as_array().unwrap().len() < 6,
        "a cut scan reads only part of the tree: {answer:#}"
    );
}

/// The site lists stop at the default limit of 50 and `has_more` says
/// so, so a Rule with many sites stays inside the page budget.
#[test]
#[verifies("rule_evidence_gives_scanned_sites", examples)]
fn evidence_cuts_the_site_lists_at_the_limit() {
    let repo = repo_with_attributes();
    let whole = evidence(repo.path(), "rule_served");
    assert_eq!(whole["meta"]["has_more"], json!(false), "{whole:#}");

    let many: String = (0..60)
        .map(|index| format!("#[rule(\"rule_served\")]\nfn serve_{index:02}() {{}}\n"))
        .collect();
    write(repo.path(), "src/many.rs", many);

    let cut = evidence(repo.path(), "rule_served");
    let data = &cut["data"];
    assert_eq!(cut["meta"]["limit"], json!(50));
    assert_eq!(
        data["implementations"].as_array().unwrap().len(),
        50,
        "{cut:#}"
    );
    assert_eq!(data["verifications"].as_array().unwrap().len(), 1);
    assert_eq!(cut["meta"]["has_more"], json!(true), "{cut:#}");
}

#[test]
#[verifies("rule_stamp_lists_a_live_word_for_uncovered_parts", examples)]
fn the_evidence_stamp_names_the_file_scan_as_live() {
    let repo = repo_with_attributes();

    let answer = evidence(repo.path(), "rule_served");

    let live = answer["meta"]["stamp"]["live"].as_array().unwrap();
    assert!(
        live.contains(&json!("scanned_sites")),
        "the attribute sites do not come from the projection: {live:?}"
    );
    let attested = answer["meta"]["stamp"]["attested"].as_array().unwrap();
    assert!(attested.contains(&json!("implementation_bindings")));
}
