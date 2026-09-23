use super::*;

#[test]
fn question_update_method_alias_is_resolved_before_schema_validation() {
    let (_directory, repo) = init();
    provenance()
        .args([
            "requirements",
            "create",
            "--repo",
            &repo,
            "--id",
            "req_question_alias",
            "--statement",
            "The system records a question.",
        ])
        .assert()
        .success();
    provenance()
        .args(["topics", "create", "--repo", &repo, "--stdin"])
        .write_stdin(
            json!({
                "id":"topic_question_alias", "requirement_id":"req_question_alias",
                "title":"Question aliases", "status":"open", "links":[]
            })
            .to_string(),
        )
        .assert()
        .success();
    provenance()
        .args([
            "questions",
            "create",
            "--repo",
            &repo,
            "--id",
            "question_alias",
            "--topic-id",
            "topic_question_alias",
            "--question",
            "Which method applies?",
            "--method",
            "research",
        ])
        .assert()
        .success();
    let output = provenance()
        .args([
            "questions",
            "question_alias",
            "update",
            "--repo",
            &repo,
            "--method",
            "grill",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(output["data"]["resolution_method"], "grill");
}
