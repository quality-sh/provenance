//! A Rule or Question answer that cites a Resolution that is not approved is refused with a
//! message that says what to do, and the routes it names succeed.
#[path = "shaping_support/fixtures.rs"]
mod fixtures;
#[path = "shaping_support/provenance.rs"]
mod provenance;

use fixtures::{create_source_and_requirement, create_topic, init};
use predicates::str::contains;
use provenance::provenance;
use provenance_macros::verifies;

const NEXT_STEP: &str = "Until a reviewer approves res_rust_with_neglect, keep the Rule in draft \
                         or review, or leave the Question open, or remove the citation and name \
                         res_rust_with_neglect in the text.";

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
        "res_rust_with_neglect",
        "--requirement-id",
        "req_overtime",
        "--title",
        "Rust shows neglect",
        "--position",
        "The robot rusts with neglect.",
        "--rationale",
        "Rust shows neglect without injury.",
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
        "question_rust",
        "--topic-id",
        "topic_overtime",
        "--question",
        "Does the robot rust with neglect?",
        "--method",
        "grill",
    ])
    .success();
    dir
}

fn rule(repo: &str, status: Option<&str>) -> assert_cmd::assert::Assert {
    let mut args = vec![
        "rules",
        "create",
        "--repo",
        repo,
        "--scope",
        "default",
        "--id",
        "rule_robot_rusts",
        "--requirement-id",
        "req_overtime",
        "--resolution-id",
        "res_rust_with_neglect",
        "--statement",
        "The robot rusts after a long time without a proven task.",
        "--severity",
        "high",
    ];
    if let Some(status) = status {
        args.extend(["--status", status]);
    }
    provenance(&args)
}

#[test]
#[verifies("rule_unapproved_resolution_refusal_says_what_to_do", examples)]
fn an_active_rule_citing_a_proposed_resolution_is_told_what_to_do() {
    let dir = setup();
    let repo = dir.path().to_str().unwrap();
    rule(repo, None)
        .failure()
        .code(1)
        .stderr(contains(r#""kind":"resolution_not_approved""#))
        .stderr(contains(r#""resolution_id":"res_rust_with_neglect""#))
        .stderr(contains(
            "rule rule_robot_rusts cites resolution res_rust_with_neglect, which is proposed.",
        ))
        .stderr(contains(NEXT_STEP));
    rule(repo, Some("review")).success();
}

#[test]
#[verifies("rule_unapproved_resolution_refusal_says_what_to_do", examples)]
fn an_answer_citing_a_proposed_resolution_is_told_what_to_do() {
    let dir = setup();
    let repo = dir.path().to_str().unwrap();
    let answer = |extra: &[&str]| {
        let mut args = vec![
            "questions",
            "question_rust",
            "answer",
            "--repo",
            repo,
            "--scope",
            "default",
            "--answer",
            "Yes. See res_rust_with_neglect.",
        ];
        args.extend_from_slice(extra);
        provenance(&args)
    };
    answer(&["--resolution-id", "res_rust_with_neglect"])
        .failure()
        .code(1)
        .stderr(contains(
            "question question_rust cites resolution res_rust_with_neglect, which is proposed.",
        ))
        .stderr(contains(NEXT_STEP));
    answer(&[]).success();
}
