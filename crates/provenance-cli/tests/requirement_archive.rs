use assert_cmd::Command;

fn run(repo: &str, args: &[&str]) -> String {
    let result = Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
        .args(args)
        .args(["--repo", repo])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout).unwrap()
}

#[test]
fn archived_requirement_is_hidden_from_default_reads_and_explicit_reads_show_it() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap();
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
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
            "req_current",
            "--statement",
            "The service keeps current records.",
        ],
    );
    run(
        repo,
        &[
            "requirements",
            "create",
            "--id",
            "req_retired",
            "--statement",
            "The service keeps a record.",
            "--refines",
            "req_current",
        ],
    );
    run(
        repo,
        &[
            "req_retired",
            "update",
            "--status",
            "archived",
            "--archived-in-commit-json",
            r#"{"commit":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}"#,
        ],
    );
    let current = run(repo, &["requirements", "list", "--format", "json"]);
    assert!(!current.contains("req_retired"), "{current}");
    assert!(run(
        repo,
        &[
            "requirements",
            "list",
            "--include-terminal",
            "--format",
            "json"
        ]
    )
    .contains("req_retired"));
    let document = run(
        repo,
        &["req_current", "--view", "review", "--format", "json"],
    );
    assert!(!document.contains("req_retired"), "{document}");
    assert!(run(
        repo,
        &[
            "api",
            "requirements/req_current/document?exclude_terminal=false",
            "--format",
            "json"
        ]
    )
    .contains("req_retired"));
    assert!(run(repo, &["req_retired", "--format", "json"]).contains("archived"));
}
