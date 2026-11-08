use super::*;

#[test]
fn question_update_method_alias_is_resolved_before_schema_validation() {
    let (_directory, repo) = init();
    provenance()
        .args([
            "req_question_alias",
            "create",
            "--type",
            "requirement",
            "--repo",
            &repo,
            "--statement",
            "The system records a question.",
        ])
        .assert()
        .success();
    provenance()
        .args([
            "topic_question_alias",
            "create",
            "--type",
            "topic",
            "--stdin",
            "--repo",
            &repo,
        ])
        .write_stdin(
            json!({
                "requirement_id":"req_question_alias",
                "title":"Question aliases", "status":"open", "links":[]
            })
            .to_string(),
        )
        .assert()
        .success();
    provenance()
        .args([
            "question_alias",
            "create",
            "--type",
            "question",
            "--repo",
            &repo,
            "--topic-id",
            "topic_question_alias",
            "--question",
            "Which method applies?",
            "--method",
            "research",
        ])
        .assert()
        .success();
    let current = provenance()
        .args([
            "api",
            "questions/question_alias",
            "--repo",
            &repo,
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    let current: Value = serde_json::from_slice(&current.stdout).unwrap();
    let etag = current["data"]["edit"]["etag"].as_str().unwrap();
    let output = provenance()
        .args([
            "question_alias",
            "update",
            "--repo",
            &repo,
            "--method",
            "ask",
            "--if-match",
            etag,
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
    assert_eq!(output["data"]["resolution_method"], "ask");
}
