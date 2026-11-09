use super::*;

// An implementation aid with no Rule binding: no Rule states the name of the Question
// method field on the CLI.
#[test]
fn question_method_is_set_with_resolution_method() {
    let (_directory, repo) = init();
    provenance()
        .args([
            "req_question_method",
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
            "topic_question_method",
            "create",
            "--type",
            "topic",
            "--stdin",
            "--repo",
            &repo,
        ])
        .write_stdin(
            json!({
                "requirement_id":"req_question_method",
                "title":"Question method", "status":"open", "links":[]
            })
            .to_string(),
        )
        .assert()
        .success();
    provenance()
        .args([
            "question_method",
            "create",
            "--type",
            "question",
            "--repo",
            &repo,
            "--topic-id",
            "topic_question_method",
            "--question",
            "Which method applies?",
            "--resolution-method",
            "research",
        ])
        .assert()
        .success();
    let current = provenance()
        .args([
            "api",
            "questions/question_method",
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
            "question_method",
            "update",
            "--repo",
            &repo,
            "--resolution-method",
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
