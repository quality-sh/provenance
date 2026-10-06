use super::*;

fn named_action_repo() -> (tempfile::TempDir, String) {
    let (directory, repo) = initialized_repo();
    json_output(&[
        "req_actions",
        "create",
        "--type",
        "requirement",
        "--repo",
        &repo,
        "--statement",
        "The named actions keep their preconditions.",
        "--format",
        "json",
    ]);
    json_output(&[
        "topic_actions",
        "create",
        "--type",
        "topic",
        "--repo",
        &repo,
        "--requirement-id",
        "req_actions",
        "--title",
        "Named actions",
        "--format",
        "json",
    ]);
    json_output(&[
        "question_actions",
        "create",
        "--type",
        "question",
        "--repo",
        &repo,
        "--topic-id",
        "topic_actions",
        "--question",
        "Does the action resolve its target?",
        "--method",
        "research",
        "--format",
        "json",
    ]);

    (directory, repo)
}

#[test]
#[verifies("rule_porcelain_existing_action_infers_kind", examples)]
#[verifies("rule_porcelain_named_domain_actions", examples)]
fn target_first_topic_actions_infer_kind_and_update_claim_state() {
    let (_directory, repo) = named_action_repo();
    let claimed = json_output(&[
        "topic_actions",
        "claim",
        "--repo",
        &repo,
        "--actor",
        "worker",
        "--format",
        "json",
    ]);
    assert_eq!(claimed["data"]["claimed_by"], "worker");
    let released = json_output(&[
        "topic_actions",
        "release",
        "--repo",
        &repo,
        "--format",
        "json",
    ]);
    assert!(released["data"]["claimed_by"].is_null());
}

#[test]
#[verifies("rule_porcelain_existing_action_infers_kind", examples)]
#[verifies("rule_porcelain_named_domain_actions", examples)]
fn target_first_question_actions_infer_kind_and_reject_a_mismatched_action() {
    let (_directory, repo) = named_action_repo();
    let answered = json_output(&[
        "question_actions",
        "answer",
        "--repo",
        &repo,
        "--answer",
        "Yes.",
        "--format",
        "json",
    ]);
    assert_eq!(answered["data"]["status"], "answered");

    provenance()
        .args([
            "question_actions",
            "claim",
            "--repo",
            &repo,
            "--actor",
            "worker",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains("unsupported action options"))
        .stderr(predicates::str::contains("unrecognized subcommand").not());
}

#[test]
#[verifies("rule_porcelain_existing_action_infers_kind", examples)]
#[verifies("rule_porcelain_named_domain_actions", examples)]
fn target_first_requirement_submit_infers_kind_and_records_the_submission() {
    let (_directory, repo) = named_action_repo();
    let current = json_output(&[
        "requirements",
        "req_actions",
        "get",
        "--repo",
        &repo,
        "--format",
        "json",
    ]);
    let automatic = current["data"]["decision"]["pending"]["proposal_id"]
        .as_str()
        .unwrap();
    json_stdin_output(
        &[
            "requirements",
            "req_actions",
            "submissions",
            automatic,
            "withdraw",
            "--repo",
            &repo,
            "--stdin",
            "--format",
            "json",
        ],
        &serde_json::json!({"actor":"agent","declared_by":null,"reason":null}),
    );
    let submitted = json_stdin_output(
        &[
            "req_actions",
            "submit",
            "--repo",
            &repo,
            "--stdin",
            "--format",
            "json",
        ],
        &serde_json::json!({
            "actor":"agent",
            "title":"CLI target",
            "summary":"The target-first action submits this Requirement.",
            "source_ids":[],
            "evidence_references":[],
            "builds_on":[]
        }),
    );
    assert_eq!(submitted["data"]["requirement_id"], "req_actions");
    assert_eq!(submitted["data"]["fact"], "submitted");
    assert!(submitted["data"]["proposal_key"]
        .as_str()
        .unwrap()
        .starts_with("review:requirement:req_actions:"));
}
