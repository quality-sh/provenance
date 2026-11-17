//! A verification binding, an implementation binding or a review whose
//! stored row passes the record budget is refused before anything is
//! written, and the evidence read of the Rule stays readable.
use provenance_macros::verifies;
use serde_json::json;

use crate::support::record_budget::{
    api, cli, refusal, repository, success, too_large, write, BUDGET,
};
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
    write(
        repo,
        &["req_data", "update", "--if-match", &etag],
        &json!({"statement": text}),
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

    let id = refused["record_id"].as_str().unwrap();
    assert!(id.starts_with("verification_binding_"), "{refused}");
    assert_eq!(refused, too_large(id, "symbol", 70_084));
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

    let refused = cli(
        repo,
        &["import"],
        &[("--input", seed.to_str().unwrap()), ("--format", "json")],
    );

    // The import command gives the text of a refusal as the message of
    // `operation_failed`.
    let message = format!(
        "record `implementation_binding_one` uses 70074 bytes and the limit is {BUDGET} bytes; \
         make the field `symbol` shorter"
    );
    assert_eq!(refused.status.code(), Some(1));
    assert_eq!(String::from_utf8_lossy(&refused.stdout), "");
    let stderr: serde_json::Value = serde_json::from_slice(&refused.stderr).unwrap();
    assert_eq!(
        stderr,
        json!({"error": {"kind": "operation_failed", "message": message}, "meta": {}})
    );
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

    let bytes = 81_580;
    assert_eq!(
        refused,
        json!({
            "kind": "record_too_large",
            "record_id": "req_data",
            "field": "statement",
            "bytes": bytes,
            "limit": BUDGET,
            "message": format!(
                "the review of the restated Requirement `req_data` uses {bytes} bytes and the \
                 limit is {BUDGET} bytes; make the field `statement` shorter"
            )
        })
    );
    assert_eq!(state_files(repo), before);
    let evidence = success(&api(repo, "rules/rule_one/evidence", None));
    assert_eq!(evidence["data"]["review_required"], true);
}
