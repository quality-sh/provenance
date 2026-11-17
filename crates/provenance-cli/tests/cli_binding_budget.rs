//! A verification binding, an implementation binding or a review whose
//! stored row passes the record budget is refused before anything is
//! written, and the evidence read of the Rule stays readable.
use provenance_macros::verifies;
use serde_json::json;

use crate::support::record_budget::{api, cli, refusal, repository, success, BUDGET};
use crate::support::review_paths::provenance;
use crate::support::state_files::state_files;

/// A statement of `paragraphs` paragraphs of six sentences that each name
/// `verb`, which the statement checker accepts.
fn statement(paragraphs: usize, verb: &str) -> String {
    let paragraph = vec![format!("The system {verb} the data."); 6].join(" ");
    vec![paragraph; paragraphs].join("\n\n")
}

/// Changes the statement of `req_data` with the etag of its edit view.
fn restate(repo: &std::path::Path, text: &str) -> std::process::Output {
    let view = success(&cli(repo, &["req_data", "get"], &[("--format", "json")]));
    let etag = view["edit"]["etag"].as_str().unwrap().to_owned();
    cli(
        repo,
        &["req_data", "update"],
        &[("--if-match", &etag), ("--statement", text)],
    )
}

#[test]
#[verifies("rule_writer_refuses_a_row_over_the_record_budget", examples)]
fn begin_verification_with_a_long_symbol_is_refused_and_writes_nothing() {
    let directory = repository();
    let repo = directory.path();
    let before = state_files(repo);
    let start = json!({
        "rule": "rule_one", "key": "check", "method": "examples",
        "declared_by": "fixture", "file": "check.rs", "symbol": "x".repeat(70_000)
    });

    let refused = refusal(&api(
        repo,
        "verification-runs/begin-verification",
        Some(&start),
    ));

    assert_eq!(refused["kind"], "record_too_large", "{refused}");
    assert!(refused["record_id"].is_string(), "{refused}");
    assert!(refused["bytes"].as_u64().unwrap() > 70_000);
    assert_eq!(refused["limit"], BUDGET);
    assert_eq!(state_files(repo), before);
    let evidence = success(&api(repo, "rules/rule_one/evidence", None));
    assert_eq!(evidence["data"]["verification_bindings"], json!([]));
}

#[test]
#[verifies("rule_writer_refuses_a_row_over_the_record_budget", examples)]
fn an_import_with_a_long_implementation_symbol_is_refused_and_writes_nothing() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path();
    let root = repo.to_str().unwrap();
    provenance()
        .args(["init", "--path", root, "--scope", "default"])
        .args(["--path-prefix", "."])
        .assert()
        .success();
    let seed = repo.join("seed.json");
    let scope = json!({
        "scope": "default",
        "requirements": [{
            "scope_id": "default", "id": "req_data",
            "statement": "The system keeps the data", "status": "discovery"
        }],
        "rules": [{
            "scope_id": "default", "id": "rule_one", "statement": "The system shows the data",
            "status": "draft", "severity": "high", "requirement_ids": ["req_data"]
        }],
        "implementation_bindings": [{
            "scope_id": "default", "id": "implementation_binding_one", "rule_id": "rule_one",
            "declared_by": "spec://typescript/data", "file": "src/data.ts",
            "symbol": "x".repeat(70_000)
        }],
        "sources": [], "resolutions": [], "threads": [], "messages": []
    });
    std::fs::write(&seed, scope.to_string()).unwrap();
    let before = state_files(repo);

    let refused = refusal(&cli(
        repo,
        &["import"],
        &[("--input", seed.to_str().unwrap()), ("--format", "json")],
    ));

    assert_eq!(refused["kind"], "record_too_large", "{refused}");
    assert_eq!(refused["record_id"], "implementation_binding_one");
    assert!(refused["bytes"].as_u64().unwrap() > 70_000);
    assert_eq!(refused["limit"], BUDGET);
    assert_eq!(state_files(repo), before);
}

#[test]
#[verifies("rule_writer_refuses_a_row_over_the_record_budget", examples)]
fn a_restatement_whose_review_passes_the_budget_is_refused_and_writes_nothing() {
    let directory = repository();
    let repo = directory.path();
    success(&restate(repo, &statement(250, "keeps")));
    let before = state_files(repo);

    let refused = refusal(&restate(repo, &statement(250, "shows")));

    assert_eq!(refused["kind"], "record_too_large", "{refused}");
    assert!(refused["record_id"].is_string(), "{refused}");
    assert!(refused["bytes"].as_u64().unwrap() > 80_000);
    assert_eq!(refused["limit"], BUDGET);
    assert_eq!(state_files(repo), before);
    let evidence = success(&api(repo, "rules/rule_one/evidence", None));
    assert_eq!(evidence["data"]["review_required"], true);
}
