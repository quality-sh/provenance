use super::{json, run, success};

#[test]
fn owned_requirement_discussions_require_the_owner_for_start_and_reply() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap();
    success(&[
        "init",
        "--path",
        repo,
        "--scope",
        "default",
        "--path-prefix",
        ".",
    ]);
    let seed = directory.path().join("seed.json");
    std::fs::write(
        &seed,
        serde_json::json!({
            "scope": "default",
            "sources": [],
            "requirements": [{
                "schema_version": 2, "scope_id": "default", "id": "req_owned",
                "statement": "The record has one owner.", "status": "discovery",
                "declared_by": "spec://owner"
            }],
            "resolutions": [], "rules": [], "threads": [], "messages": []
        })
        .to_string(),
    )
    .unwrap();
    success(&["import", "--repo", repo, "--input", seed.to_str().unwrap()]);

    for owner in [None, Some("spec://other")] {
        let mut args = vec!["req_owned", "discuss", "--repo", repo, "--body", "Question"];
        if let Some(owner) = owner {
            args.extend(["--declared-by", owner]);
        }
        let output = run(&args);
        assert!(!output.status.success(), "{args:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains("declared_by must match the existing owner"),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(json(&[
            "req_owned",
            "discussions",
            "--repo",
            repo,
            "--format",
            "json"
        ])["result"]["entries"]
            .as_array()
            .unwrap()
            .is_empty());
    }

    let start = json(&[
        "req_owned",
        "discuss",
        "--repo",
        repo,
        "--body",
        "Question",
        "--declared-by",
        "spec://owner",
        "--format",
        "json",
    ]);
    let id = start["receipt"]["discussion_id"].as_str().unwrap();
    assert_eq!(start["receipt"]["version"], 1);

    for owner in [None, Some("spec://other")] {
        let mut args = vec![
            id,
            "reply",
            "--repo",
            repo,
            "--body",
            "Answer",
            "--expected-version",
            "1",
        ];
        if let Some(owner) = owner {
            args.extend(["--declared-by", owner]);
        }
        let output = run(&args);
        assert!(!output.status.success(), "{args:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains("declared_by must match the existing owner"),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let conversation = json(&["discussions", id, "get", "--repo", repo, "--format", "json"]);
        assert_eq!(conversation["result"]["head"]["version"], 1);
        assert_eq!(
            conversation["result"]["messages"]["entries"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }

    let reply = json(&[
        id,
        "reply",
        "--repo",
        repo,
        "--body",
        "Answer",
        "--expected-version",
        "1",
        "--declared-by",
        "spec://owner",
        "--format",
        "json",
    ]);
    assert_eq!(reply["receipt"]["version"], 2);
    let conversation = json(&["discussions", id, "get", "--repo", repo, "--format", "json"]);
    assert_eq!(
        conversation["result"]["messages"]["entries"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn topic_and_question_discussions_refuse_a_declared_owner() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap();
    success(&[
        "init",
        "--path",
        repo,
        "--scope",
        "default",
        "--path-prefix",
        ".",
    ]);
    success(&[
        "requirements",
        "create",
        "--repo",
        repo,
        "--id",
        "req_topic",
        "--statement",
        "The topic has a requirement.",
    ]);
    success(&[
        "topics",
        "create",
        "--repo",
        repo,
        "--id",
        "topic_ownerless",
        "--requirement-id",
        "req_topic",
        "--title",
        "Ownerless topic",
    ]);
    success(&[
        "questions",
        "create",
        "--repo",
        repo,
        "--id",
        "question_ownerless",
        "--topic-id",
        "topic_ownerless",
        "--question",
        "Which record applies?",
        "--method",
        "research",
    ]);

    for id in ["topic_ownerless", "question_ownerless"] {
        let output = run(&[
            id,
            "discuss",
            "--repo",
            repo,
            "--body",
            "Question",
            "--declared-by",
            "spec://owner",
        ]);
        assert!(!output.status.success(), "{id}");
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains("a topic or question parent takes no declared owner"),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            json(&[id, "discussions", "--repo", repo, "--format", "json"])["result"]["entries"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            json(&[id, "discuss", "--repo", repo, "--body", "Question", "--format", "json"])
                ["receipt"]["version"],
            1
        );
    }
}
