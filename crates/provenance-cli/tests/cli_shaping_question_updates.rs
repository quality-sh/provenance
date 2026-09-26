#[path = "shaping_support/fixtures.rs"]
mod fixtures;
#[path = "shaping_support/provenance.rs"]
mod provenance;

use fixtures::{create_source_and_requirement, create_topic, init};
use predicates::str::contains;
use provenance::{provenance, provenance_stdin};

#[test]
fn cli_questions_update_changes_method_status_links_and_resolution_id() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo").to_string_lossy().to_string();

    init(&repo);
    create_source_and_requirement(&repo);
    create_topic(&repo);
    create_claimed_fork_question(&repo);
    create_resolution(&repo);
    update_question_to_blocked_prototype(&repo);
    question_update_rejects_invalid_links(&repo);
    question_update_rejects_answered_without_answer(&repo);
}

fn create_claimed_fork_question(repo: &str) {
    provenance(&[
        "questions",
        "create",
        "--repo",
        repo,
        "--scope",
        "default",
        "--id",
        "question_fork",
        "--topic-id",
        "topic_overtime",
        "--question",
        "Which UI direction should the shaping map use?",
        "--method",
        "grill",
        "--format",
        "json",
    ])
    .success();
    provenance(&[
        "questions",
        "question_fork",
        "claim",
        "--repo",
        repo,
        "--scope",
        "default",
        "--actor",
        "agent-one",
        "--format",
        "json",
    ])
    .success()
    .stdout(contains(r#""claimed_by": "agent-one""#));
}

fn create_resolution(repo: &str) {
    provenance(&[
        "resolutions",
        "create",
        "--repo",
        repo,
        "--scope",
        "default",
        "--id",
        "res_overtime",
        "--title",
        "Overtime threshold",
        "--requirement-id",
        "req_overtime",
        "--position",
        "Use the SCHADS threshold.",
        "--rationale",
        "Human confirmed the source threshold.",
        "--format",
        "json",
    ])
    .success()
    .stdout(contains("res_overtime"));
}

fn update_question_to_blocked_prototype(repo: &str) {
    let updated = provenance_stdin(&[
        "questions",
        "question_fork",
        "update",
        "--repo",
        repo,
        "--scope",
        "default",
        "--stdin",
        "--format",
        "json",
    ], r#"{"resolution_method":"prototype","status":"blocked_on_human","links":[{"target_type":"source","target_id":"source_schads"},{"target_type":"resolution","target_id":"res_overtime"}],"resolution_id":"res_overtime"}"#)
    .success()
    .stdout(contains(r#""resolution_method": "prototype""#))
    .stdout(contains(r#""status": "blocked_on_human""#))
    .stdout(contains(r#""target_type": "source""#))
    .stdout(contains(r#""target_type": "resolution""#))
    .stdout(contains(r#""resolution_id": "res_overtime""#));
    let updated_stdout = String::from_utf8(updated.get_output().stdout.clone()).unwrap();
    assert!(!updated_stdout.contains("claimed_by"));

    provenance(&[
        "questions",
        "question_fork",
        "claim",
        "--repo",
        repo,
        "--scope",
        "default",
        "--actor",
        "agent-two",
    ])
    .failure()
    .stderr(contains("invalid_update"));
}

fn question_update_rejects_invalid_links(repo: &str) {
    provenance_stdin(
        &[
            "questions",
            "question_fork",
            "update",
            "--repo",
            repo,
            "--scope",
            "default",
            "--stdin",
        ],
        r#"{"links":[{"target_type":"source","target_id":"missing_source"}]}"#,
    )
    .failure()
    .stderr(contains("missing_reference"));
}

fn question_update_rejects_answered_without_answer(repo: &str) {
    provenance(&[
        "questions",
        "create",
        "--repo",
        repo,
        "--scope",
        "default",
        "--id",
        "question_unanswered",
        "--topic-id",
        "topic_overtime",
        "--question",
        "What still needs a real answer?",
        "--method",
        "grill",
        "--format",
        "json",
    ])
    .success();
    provenance_stdin(
        &[
            "questions",
            "question_unanswered",
            "update",
            "--repo",
            repo,
            "--scope",
            "default",
            "--stdin",
        ],
        r#"{"status":"answered"}"#,
    )
    .failure()
    .stderr(contains("invalid_update"));
}
