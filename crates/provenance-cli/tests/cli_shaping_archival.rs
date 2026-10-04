use assert_cmd::Command;
use serde_json::Value;

fn run(repo: &str, args: &[&str]) -> Value {
    let output = Command::cargo_bin("provenance")
        .unwrap()
        .args(args)
        .args(["--repo", repo, "--scope", "default", "--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    serde_json::from_slice(&output).unwrap()
}

fn archive(repo: &str, family: &str, id: &str) -> Value {
    let current = run(repo, &[family, id, "get"]);
    let etag = current["data"]["edit"]["etag"].as_str().unwrap();
    let output = Command::cargo_bin("provenance")
        .unwrap()
        .args([
            family,
            id,
            "update",
            "--repo",
            repo,
            "--scope",
            "default",
            "--if-match",
            etag,
            "--stdin",
            "--format",
            "json",
        ])
        .write_stdin(format!(
            r#"{{"status":"archived","archived_in_commit":{{"commit":"{}"}}}}"#,
            "a".repeat(40)
        ))
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    serde_json::from_slice(&output).unwrap()
}

fn initialized_records() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap();
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "init",
            "--path",
            repo,
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();
    run(
        repo,
        &[
            "requirements",
            "create",
            "--id",
            "req_archive",
            "--statement",
            "The system keeps history.",
        ],
    );
    run(
        repo,
        &[
            "topics",
            "create",
            "--id",
            "topic_archive",
            "--requirement-id",
            "req_archive",
            "--title",
            "Archive topic",
        ],
    );
    for id in ["question_one", "question_two"] {
        run(
            repo,
            &[
                "questions",
                "create",
                "--id",
                id,
                "--topic-id",
                "topic_archive",
                "--question",
                "Keep this history?",
                "--method",
                "research",
            ],
        );
    }
    directory
}

#[test]
fn archiving_a_question_keeps_its_topic_and_discussion_history() {
    let directory = initialized_records();
    let repo = directory.path().to_str().unwrap();
    let discussion = run(
        repo,
        &["question_one", "discuss", "--body", "Keep this message."],
    );
    let discussion_id = discussion["receipt"]["discussion_id"].as_str().unwrap();

    let archived = archive(repo, "questions", "question_one");
    assert_eq!(archived["data"]["status"], "archived");
    assert_eq!(
        run(repo, &["topics", "topic_archive", "get"])["data"]["status"],
        "open"
    );
    assert_eq!(
        run(repo, &["discussions", discussion_id, "get"])["result"]["messages"]["entries"][0]
            ["body"],
        "Keep this message."
    );
}

#[test]
fn archiving_a_topic_archives_all_of_its_questions_in_one_update() {
    let directory = initialized_records();
    let repo = directory.path().to_str().unwrap();
    let topic = archive(repo, "topics", "topic_archive");
    assert_eq!(topic["data"]["status"], "archived");
    for id in ["question_one", "question_two"] {
        let question = run(repo, &["questions", id, "get"]);
        assert_eq!(question["data"]["status"], "archived");
        assert_eq!(
            question["data"]["archived_in_commit"]["commit"],
            "a".repeat(40)
        );
    }
}

#[test]
fn topic_and_question_updates_require_the_current_etag() {
    let directory = initialized_records();
    let repo = directory.path().to_str().unwrap();
    for (family, id) in [("topics", "topic_archive"), ("questions", "question_one")] {
        Command::cargo_bin("provenance")
            .unwrap()
            .args([
                family, id, "update", "--repo", repo, "--scope", "default", "--stdin", "--format",
                "json",
            ])
            .write_stdin(r#"{"title":"No unguarded update"}"#)
            .assert()
            .failure()
            .stderr(predicates::str::contains("If-Match"));
    }
    assert_eq!(
        run(repo, &["topics", "topic_archive", "get"])["data"]["status"],
        "open"
    );
    assert_eq!(
        run(repo, &["questions", "question_one", "get"])["data"]["status"],
        "open"
    );
}
