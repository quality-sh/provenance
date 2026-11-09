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
    create_topic(&repo);
    create_question(&repo);
    post_topic_thread(&repo);
    post_question_thread(&repo);
    verify_materialized_lists(&repo);
}

fn create_question(repo: &str) {
    provenance(&[
        "question_overtime_threshold",
        "create",
        "--type",
        "question",
        "--repo",
        repo,
        "--scope",
        "default",
        "--topic-id",
        "topic_overtime",
        "--question",
        "Which threshold applies?",
        "--resolution-method",
        "ask",
        "--status",
        "answered",
        "--answer",
        "Use the SCHADS overtime threshold.",
        "--format",
        "json",
    ])
    .success()
    .stdout(contains("question_overtime_threshold"))
    .stdout(contains(r#""resolution_method": "ask""#))
    .stdout(contains(r#""requirement_id": "req_overtime""#));
}

fn post_topic_thread(repo: &str) {
    provenance_stdin(
        &[
            "api",
            "topics/topic_overtime/discussions",
            "--method",
            "post",
            "--input",
            "-",
            "--repo",
            repo,
            "--scope",
            "default",
            "--format",
            "json",
        ],
        r#"{"actor":"cli","role":"assistant","body":"Work this topic before shaping the pitch"}"#,
    )
    .success()
    .stdout(contains("discussion"));
}

fn post_question_thread(repo: &str) {
    provenance_stdin(
        &[
            "api",
            "questions/question_overtime_threshold/discussions",
            "--method",
            "post",
            "--input",
            "-",
            "--repo",
            repo,
            "--scope",
            "default",
            "--format",
            "json",
        ],
        r#"{"actor":"cli","role":"assistant","body":"Answered from SCHADS clause 28.1"}"#,
    )
    .success()
    .stdout(contains("discussion"));
}

fn verify_materialized_lists(repo: &str) {
    provenance(&["materialize", "--repo", repo, "--format", "json"])
        .success()
        .stdout(contains(r#""records_loaded""#));
    provenance(&[
        "api", "topics", "--repo", repo, "--scope", "default", "--format", "json",
    ])
    .success()
    .stdout(contains("topic_overtime"));
    provenance(&[
        "api",
        "questions",
        "--repo",
        repo,
        "--scope",
        "default",
        "--format",
        "json",
    ])
    .success()
    .stdout(contains("question_overtime_threshold"));
}
