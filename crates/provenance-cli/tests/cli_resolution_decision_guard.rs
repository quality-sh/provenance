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
        "resolutions",
        "create",
        "--repo",
        repo,
        "--scope",
        "default",
        "--id",
        "res_code_scores_health",
        "--requirement-id",
        "req_overtime",
        "--title",
        "Code scores affect health",
        "--position",
        "Code scores affect health.",
        "--rationale",
        "The owner must decide.",
        "--status",
        "proposed",
    ])
    .success();
    provenance(&[
        "questions",
        "create",
        "--repo",
        repo,
        "--scope",
        "default",
        "--id",
        "question_code_scores_health",
        "--topic-id",
        "topic_overtime",
        "--question",
        "Do code scores affect health?",
        "--method",
        "grill",
    ])
    .success();
    dir
}

#[test]
fn cli_answer_refuses_proposed_resolution_and_keeps_question_open() {
    let dir = setup();
    let repo = dir.path().to_str().unwrap();
    let result = provenance(&[
        "questions",
        "question_code_scores_health",
        "answer",
        "--repo",
        repo,
        "--scope",
        "default",
        "--answer",
        "Code scores affect health.",
        "--resolution-id",
        "res_code_scores_health",
        "--format",
        "json",
    ]);
    eprintln!(
        "Before/After CLI: {}",
        String::from_utf8_lossy(&result.get_output().stdout)
    );
    eprintln!(
        "Before/After diagnostic: {}",
        String::from_utf8_lossy(&result.get_output().stderr)
    );
    result
        .failure()
        .code(1)
        .stderr(contains(r#""kind":"invalid_update""#));
    provenance(&[
        "questions",
        "question_code_scores_health",
        "get",
        "--repo",
        repo,
        "--scope",
        "default",
        "--format",
        "json",
    ])
    .success()
    .stdout(contains(r#""status": "open""#));
}

#[test]
fn cli_active_rule_refuses_proposed_resolution() {
    let dir = setup();
    let repo = dir.path().to_str().unwrap();
    provenance_stdin(&[
        "rules", "create", "--repo", repo, "--scope", "default", "--stdin",
    ], r#"{"id":"rule_code_scores_health","statement":"Code scores affect health.","status":"active","severity":"high","requirement_ids":["req_overtime"],"resolution_ids":["res_code_scores_health"]}"#)
        .failure().code(1).stderr(contains(r#""kind":"invalid_update""#));
}

fn edit(repo: &str, kind: &str, id: &str, body: &str) -> assert_cmd::assert::Assert {
    let mut args = vec![
        kind, id, "update", "--repo", repo, "--scope", "default", "--stdin", "--format", "json",
    ];
    let etag;
    if kind == "questions" {
        let output = provenance(&[
            kind, id, "get", "--repo", repo, "--scope", "default", "--format", "json",
        ])
        .success()
        .get_output()
        .stdout
        .clone();
        let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
        etag = value["data"]["edit"]["etag"].as_str().unwrap().to_owned();
        args.extend(["--if-match", &etag]);
    }
    provenance_stdin(&args, body)
}

#[test]
fn cli_answer_checks_existing_resolution_link() {
    let dir = setup();
    let repo = dir.path().to_str().unwrap();
    edit(
        repo,
        "questions",
        "question_code_scores_health",
        r#"{"resolution_id":"res_code_scores_health"}"#,
    )
    .success();
    provenance(&[
        "questions",
        "question_code_scores_health",
        "answer",
        "--repo",
        repo,
        "--scope",
        "default",
        "--answer",
        "Code scores affect health.",
    ])
    .failure()
    .code(1)
    .stderr(contains(r#""kind":"invalid_update""#));
}

#[test]
fn cli_answered_question_creation_refuses_proposed_resolution() {
    let dir = setup();
    let repo = dir.path().to_str().unwrap();
    provenance(&[
        "questions",
        "create",
        "--repo",
        repo,
        "--scope",
        "default",
        "--id",
        "question_daily_health_budget",
        "--topic-id",
        "topic_overtime",
        "--question",
        "What is the daily health budget?",
        "--method",
        "grill",
        "--status",
        "answered",
        "--answer",
        "Use the proposed budget.",
        "--resolution-id",
        "res_code_scores_health",
    ])
    .failure()
    .code(1)
    .stderr(contains(r#""kind":"invalid_update""#));
}

#[test]
fn cli_answered_question_edit_refuses_proposed_resolution() {
    let dir = setup();
    let repo = dir.path().to_str().unwrap();
    provenance(&[
        "questions",
        "question_code_scores_health",
        "answer",
        "--repo",
        repo,
        "--scope",
        "default",
        "--answer",
        "Code scores affect health.",
    ])
    .success();
    edit(
        repo,
        "questions",
        "question_code_scores_health",
        r#"{"resolution_id":"res_code_scores_health"}"#,
    )
    .failure()
    .code(1)
    .stderr(contains(r#""kind":"invalid_update""#));
}

#[test]
fn cli_active_rule_edit_refuses_proposed_resolution() {
    let dir = setup();
    let repo = dir.path().to_str().unwrap();
    provenance_stdin(&[
        "rules", "create", "--repo", repo, "--scope", "default", "--stdin",
    ], r#"{"id":"rule_code_scores_health","statement":"Code scores affect health.","status":"draft","severity":"high","requirement_ids":["req_overtime"],"resolution_ids":["res_code_scores_health"]}"#).success();
    edit(
        repo,
        "rules",
        "rule_code_scores_health",
        r#"{"status":"active"}"#,
    )
    .failure()
    .code(1)
    .stderr(contains(r#""kind":"invalid_update""#));
    provenance(&[
        "rules",
        "rule_code_scores_health",
        "get",
        "--repo",
        repo,
        "--scope",
        "default",
        "--format",
        "json",
    ])
    .success()
    .stdout(contains(r#""status": "draft""#));
}

#[test]
fn cli_approved_resolution_allows_answer_and_active_rule() {
    let dir = setup();
    let repo = dir.path().to_str().unwrap();
    edit(
        repo,
        "resolutions",
        "res_code_scores_health",
        r#"{"status":"approved"}"#,
    )
    .success();
    provenance(&[
        "questions",
        "question_code_scores_health",
        "answer",
        "--repo",
        repo,
        "--scope",
        "default",
        "--answer",
        "Code scores affect health.",
        "--resolution-id",
        "res_code_scores_health",
        "--format",
        "json",
    ])
    .success()
    .stdout(contains(r#""status": "answered""#));
    provenance_stdin(&[
        "rules", "create", "--repo", repo, "--scope", "default", "--stdin",
    ], r#"{"id":"rule_code_scores_health","statement":"Code scores affect health.","status":"active","severity":"high","requirement_ids":["req_overtime"],"resolution_ids":["res_code_scores_health"]}"#).success();
}

#[test]
fn cli_resolution_cannot_lose_approval_while_it_answers_a_question() {
    let dir = setup();
    let repo = dir.path().to_str().unwrap();
    edit(
        repo,
        "resolutions",
        "res_code_scores_health",
        r#"{"status":"approved"}"#,
    )
    .success();
    provenance(&[
        "questions",
        "question_code_scores_health",
        "answer",
        "--repo",
        repo,
        "--scope",
        "default",
        "--answer",
        "Code scores affect health.",
        "--resolution-id",
        "res_code_scores_health",
    ])
    .success();
    edit(
        repo,
        "resolutions",
        "res_code_scores_health",
        r#"{"status":"rejected"}"#,
    )
    .failure()
    .code(1)
    .stderr(contains(r#""kind":"invalid_update""#));
    edit(
        repo,
        "questions",
        "question_code_scores_health",
        r#"{"status":"open"}"#,
    )
    .success();
    edit(
        repo,
        "resolutions",
        "res_code_scores_health",
        r#"{"status":"rejected"}"#,
    )
    .success();
}

#[test]
fn cli_import_refuses_answered_question_with_proposed_resolution() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    let repo = repo.to_str().unwrap();
    init(repo);
    let mut document = serde_json::json!({
        "scope":"default", "sources":[], "rules":[], "threads":[], "messages":[],
        "requirements":[{"id":"req_health","statement":"Code scores affect health.","status":"discovery"}],
        "resolutions":[{"id":"res_code_scores_health","title":"Code scores affect health",
            "position":"Code scores affect health.","rationale":"The owner must decide.",
            "status":"proposed","requirement_ids":["req_health"]}],
        "topics":[{"id":"topic_health","requirement_id":"req_health","title":"Health","status":"open","links":[]}],
        "questions":[{"id":"question_code_scores_health","topic_id":"topic_health",
            "requirement_id":"req_health","question":"Do code scores affect health?",
            "resolution_method":"grill","status":"answered","answer":"Code scores affect health.",
            "resolution_id":"res_code_scores_health","links":[]}]
    });
    for family in ["requirements", "resolutions", "topics", "questions"] {
        for record in document[family].as_array_mut().unwrap() {
            record["schema_version"] =
                serde_json::json!(provenance_core::SUPPORTED_SCHEMA_VERSION.0);
            record["scope_id"] = serde_json::json!("default");
        }
    }
    let input = dir.path().join("rehearsal.json");
    std::fs::write(&input, document.to_string()).unwrap();
    provenance(&[
        "import",
        "--repo",
        repo,
        "--scope",
        "default",
        "--input",
        input.to_str().unwrap(),
    ])
    .failure()
    .code(1)
    .stderr(contains(r#""kind":"invalid_update""#));
}

#[test]
fn cli_answer_refuses_each_unapproved_resolution_status() {
    let dir = setup();
    let repo = dir.path().to_str().unwrap();
    for status in [
        "draft",
        "review",
        "proposed",
        "rejected",
        "revised",
        "superseded",
        "abandoned",
    ] {
        let patch = serde_json::json!({"status":status}).to_string();
        edit(repo, "resolutions", "res_code_scores_health", &patch).success();
        provenance(&[
            "questions",
            "question_code_scores_health",
            "answer",
            "--repo",
            repo,
            "--scope",
            "default",
            "--answer",
            "Code scores affect health.",
            "--resolution-id",
            "res_code_scores_health",
        ])
        .failure()
        .code(1)
        .stderr(contains(r#""kind":"invalid_update""#));
    }
}

#[test]
fn cli_resolution_cannot_lose_approval_while_it_produces_an_active_rule() {
    let dir = setup();
    let repo = dir.path().to_str().unwrap();
    edit(
        repo,
        "resolutions",
        "res_code_scores_health",
        r#"{"status":"approved"}"#,
    )
    .success();
    provenance_stdin(&[
        "rules", "create", "--repo", repo, "--scope", "default", "--stdin",
    ], r#"{"id":"rule_code_scores_health","statement":"Code scores affect health.","status":"active","severity":"high","requirement_ids":["req_overtime"],"resolution_ids":["res_code_scores_health"]}"#).success();
    edit(
        repo,
        "resolutions",
        "res_code_scores_health",
        r#"{"status":"proposed"}"#,
    )
    .failure()
    .code(1)
    .stderr(contains(r#""kind":"invalid_update""#));
    edit(
        repo,
        "rules",
        "rule_code_scores_health",
        r#"{"status":"draft"}"#,
    )
    .success();
    edit(
        repo,
        "resolutions",
        "res_code_scores_health",
        r#"{"status":"proposed"}"#,
    )
    .success();
}
