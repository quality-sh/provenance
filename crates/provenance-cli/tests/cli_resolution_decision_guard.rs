//! Regression tests for provenance-ps18. No matching Rule exists in the graph.
#[path = "shaping_support/fixtures.rs"]
mod fixtures;
#[path = "shaping_support/provenance.rs"]
mod provenance;

use fixtures::{create_source_and_requirement, create_topic, init};
use predicates::str::contains;
use provenance::{provenance, provenance_stdin};

fn setup() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().to_str().unwrap();
    init(repo);
    create_source_and_requirement(repo);
    create_topic(repo);
    provenance(&[
        "resolutions", "create", "--repo", repo, "--scope", "default",
        "--id", "res_code_scores_health", "--requirement-id", "req_overtime",
        "--title", "Code scores affect health", "--position", "Code scores affect health.",
        "--rationale", "The owner must decide.", "--status", "proposed",
    ]).success();
    provenance(&[
        "questions", "create", "--repo", repo, "--scope", "default",
        "--id", "question_code_scores_health", "--topic-id", "topic_overtime",
        "--question", "Do code scores affect health?", "--method", "grill",
    ]).success();
    dir
}

#[test]
fn cli_answer_refuses_proposed_resolution_and_keeps_question_open() {
    let dir = setup();
    let repo = dir.path().to_str().unwrap();
    let result = provenance(&[
        "questions", "question_code_scores_health", "answer", "--repo", repo,
        "--scope", "default", "--answer", "Code scores affect health.",
        "--resolution-id", "res_code_scores_health", "--format", "json",
    ]);
    eprintln!("Before/After CLI: {}", String::from_utf8_lossy(&result.get_output().stdout));
    eprintln!("Before/After diagnostic: {}", String::from_utf8_lossy(&result.get_output().stderr));
    result.failure().stderr(contains("invalid_update")).stderr(contains("must be approved"));
    provenance(&[
        "questions", "question_code_scores_health", "get", "--repo", repo,
        "--scope", "default", "--format", "json",
    ]).success().stdout(contains(r#""status": "open""#));
}

#[test]
fn cli_active_rule_refuses_proposed_resolution() {
    let dir = setup();
    let repo = dir.path().to_str().unwrap();
    provenance_stdin(&[
        "rules", "create", "--repo", repo, "--scope", "default", "--stdin",
    ], r#"{"id":"rule_code_scores_health","statement":"Code scores affect health.","status":"active","severity":"high","requirement_ids":["req_overtime"],"resolution_ids":["res_code_scores_health"]}"#)
        .failure().stderr(contains("must be approved"));
}

fn edit(repo: &str, kind: &str, id: &str, body: &str) -> assert_cmd::assert::Assert {
    let output = provenance(&[
        kind, id, "get", "--repo", repo, "--scope", "default", "--format", "json",
    ]).success().get_output().stdout.clone();
    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let etag = value["data"]["edit"]["etag"].as_str().unwrap();
    provenance_stdin(&[
        kind, id, "update", "--repo", repo, "--scope", "default",
        "--if-match", etag, "--stdin", "--format", "json",
    ], body)
}

#[test]
fn cli_answer_checks_existing_resolution_link() {
    let dir = setup();
    let repo = dir.path().to_str().unwrap();
    edit(repo, "questions", "question_code_scores_health",
        r#"{"resolution_id":"res_code_scores_health"}"#).success();
    provenance(&[
        "questions", "question_code_scores_health", "answer", "--repo", repo,
        "--scope", "default", "--answer", "Code scores affect health.",
    ]).failure().stderr(contains("must be approved"));
}

#[test]
fn cli_answered_question_creation_refuses_proposed_resolution() {
    let dir = setup();
    let repo = dir.path().to_str().unwrap();
    provenance(&[
        "questions", "create", "--repo", repo, "--scope", "default",
        "--id", "question_daily_health_budget", "--topic-id", "topic_overtime",
        "--question", "What is the daily health budget?", "--method", "grill",
        "--status", "answered", "--answer", "Use the proposed budget.",
        "--resolution-id", "res_code_scores_health",
    ]).failure().stderr(contains("must be approved"));
}

#[test]
fn cli_answered_question_edit_refuses_proposed_resolution() {
    let dir = setup();
    let repo = dir.path().to_str().unwrap();
    provenance(&[
        "questions", "question_code_scores_health", "answer", "--repo", repo,
        "--scope", "default", "--answer", "Code scores affect health.",
    ]).success();
    edit(repo, "questions", "question_code_scores_health",
        r#"{"resolution_id":"res_code_scores_health"}"#)
        .failure().stderr(contains("must be approved"));
}

#[test]
fn cli_active_rule_edit_refuses_proposed_resolution() {
    let dir = setup();
    let repo = dir.path().to_str().unwrap();
    provenance_stdin(&[
        "rules", "create", "--repo", repo, "--scope", "default", "--stdin",
    ], r#"{"id":"rule_code_scores_health","statement":"Code scores affect health.","status":"draft","severity":"high","requirement_ids":["req_overtime"],"resolution_ids":["res_code_scores_health"]}"#).success();
    edit(repo, "rules", "rule_code_scores_health", r#"{"status":"active"}"#)
        .failure().stderr(contains("must be approved"));
    provenance(&[
        "rules", "rule_code_scores_health", "get", "--repo", repo,
        "--scope", "default", "--format", "json",
    ]).success().stdout(contains(r#""status": "draft""#));
}

#[test]
fn cli_approved_resolution_allows_answer_and_active_rule() {
    let dir = setup();
    let repo = dir.path().to_str().unwrap();
    edit(repo, "resolutions", "res_code_scores_health", r#"{"status":"approved"}"#).success();
    provenance(&[
        "questions", "question_code_scores_health", "answer", "--repo", repo,
        "--scope", "default", "--answer", "Code scores affect health.",
        "--resolution-id", "res_code_scores_health", "--format", "json",
    ]).success().stdout(contains(r#""status": "answered""#));
    provenance_stdin(&[
        "rules", "create", "--repo", repo, "--scope", "default", "--stdin",
    ], r#"{"id":"rule_code_scores_health","statement":"Code scores affect health.","status":"active","severity":"high","requirement_ids":["req_overtime"],"resolution_ids":["res_code_scores_health"]}"#).success();
}
