use assert_cmd::Command;
use serde_json::Value;

fn run(repo: &str, args: &[&str]) -> String {
    let output = Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
        .args(args)
        .args(["--repo", repo])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8(output.stdout).unwrap()
}

// Regression evidence for the reported search fault; no graph Rule was added.
#[test]
fn search_shows_lifecycle_and_keeps_archived_records_hidden() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap();
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
        .args(["init", "--path", repo, "--scope", "default", "--path-prefix", "."])
        .assert().success();
    run(repo, &["requirements", "create", "--id", "req_search", "--statement", "The system is safe."]);
    run(repo, &["rules", "create", "--id", "rule_search", "--requirement-id", "req_search", "--statement", "The system is safe.", "--severity", "medium", "--status", "deprecated"]);
    run(repo, &["topics", "create", "--id", "topic_search", "--requirement-id", "req_search", "--title", "Search topic"]);
    run(repo, &["questions", "create", "--id", "question_search", "--topic-id", "topic_search", "--question", "Which state applies?", "--method", "research", "--status", "answered", "--answer", "Use the current state."]);

    let readable = run(repo, &["search"]);
    for expected in ["requirement req_search · active", "rule rule_search · deprecated", "topic topic_search · open", "question question_search · answered"] {
        assert!(readable.contains(expected), "{readable}");
    }
    let page: Value = serde_json::from_str(&run(repo, &["search", "--format", "json"])).unwrap();
    assert_eq!(page["nodes"][1]["status"], "deprecated");
    assert_eq!(page["nodes"][3]["status"], "answered");

    let stamp = r#"{"commit":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}"#;
    for id in ["rule_search", "question_search"] {
        run(repo, &[id, "update", "--status", "archived", "--archived-in-commit-json", stamp]);
    }
    let readable = run(repo, &["search"]);
    assert!(!readable.contains("rule_search"), "{readable}");
    assert!(!readable.contains("question_search"), "{readable}");
}
