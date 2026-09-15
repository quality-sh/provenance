#[path = "shaping_support/fixtures.rs"]
mod fixtures;
#[path = "shaping_support/provenance.rs"]
mod provenance;

use fixtures::{create_source_and_requirement, create_topic, init};
use predicates::str::contains;
use provenance::{provenance, provenance_stdin};

#[test]
fn cli_shaping_records_materialize_and_accept_topic_question_discussions() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo").to_string_lossy().to_string();

    init(&repo);
    create_source_and_requirement(&repo);
    create_boundary(&repo);
    create_topic(&repo);
    create_question(&repo);
    post_topic_thread(&repo);
    post_question_thread(&repo);
    verify_materialized_lists(&repo);
}

fn create_boundary(repo: &str) {
    provenance_stdin(&[
        "boundaries",
        "create",
        "--repo",
        repo,
        "--scope",
        "default",
        "--stdin",
        "--format",
        "json",
    ], r#"{"id":"boundary_no_manual_rework","requirement_id":"req_overtime","statement":"No manual payroll reconciliation","source_ref":{"source_id":"source_schads","clause":"28.1"}}"#)
    .success()
    .stdout(contains("boundary_no_manual_rework"))
    .stdout(contains(r#""source_id": "source_schads""#));
}

fn create_question(repo: &str) {
    provenance(&[
        "questions",
        "create",
        "--repo",
        repo,
        "--scope",
        "default",
        "--id",
        "question_overtime_threshold",
        "--topic-id",
        "topic_overtime",
        "--question",
        "Which threshold applies?",
        "--method",
        "grill",
        "--status",
        "answered",
        "--answer",
        "Use the SCHADS overtime threshold.",
        "--format",
        "json",
    ])
    .success()
    .stdout(contains("question_overtime_threshold"))
    .stdout(contains(r#""resolution_method": "grill""#))
    .stdout(contains(r#""requirement_id": "req_overtime""#));
}

fn post_topic_thread(repo: &str) {
    provenance_stdin(
        &[
            "topics",
            "topic_overtime",
            "discussions",
            "create",
            "--repo",
            repo,
            "--scope",
            "default",
            "--stdin",
            "--format",
            "json",
        ],
        r#"{"role":"assistant","body":"Work this topic before shaping the pitch"}"#,
    )
    .success()
    .stdout(contains("discussion"));
}

fn post_question_thread(repo: &str) {
    provenance_stdin(
        &[
            "questions",
            "question_overtime_threshold",
            "discussions",
            "create",
            "--repo",
            repo,
            "--scope",
            "default",
            "--stdin",
            "--format",
            "json",
        ],
        r#"{"role":"assistant","body":"Answered from SCHADS clause 28.1"}"#,
    )
    .success()
    .stdout(contains("discussion"));
}

fn verify_materialized_lists(repo: &str) {
    provenance(&["materialize", "--repo", repo, "--format", "json"])
        .success()
        .stdout(contains(r#""records_loaded""#));
    provenance(&[
        "topics", "list", "--repo", repo, "--scope", "default", "--format", "json",
    ])
    .success()
    .stdout(contains("topic_overtime"));
    provenance(&[
        "questions",
        "list",
        "--repo",
        repo,
        "--scope",
        "default",
        "--format",
        "json",
    ])
    .success()
    .stdout(contains("question_overtime_threshold"));
    provenance(&[
        "boundaries",
        "list",
        "--repo",
        repo,
        "--scope",
        "default",
        "--format",
        "json",
    ])
    .success()
    .stdout(contains("boundary_no_manual_rework"));
}
