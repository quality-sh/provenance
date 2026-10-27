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

fn listed_ids(output: &str) -> Vec<String> {
    let listed: serde_json::Value = serde_json::from_str(output).unwrap();
    listed["data"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["id"].as_str().unwrap().to_owned())
        .collect()
}

/// Creates `req_current` and archives its child `req_retired`.
fn archived_repository() -> tempfile::TempDir {
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
            "The system is safe.",
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
            "The system is safe.",
            "--refines",
            "req_current",
        ],
    );
    let current: serde_json::Value = serde_json::from_str(&run(
        repo,
        &["api", "requirements/req_retired", "--format", "json"],
    ))
    .unwrap();
    let etag = current["data"]["edit"]["etag"].as_str().unwrap();
    run(
        repo,
        &[
            "req_retired",
            "update",
            "--if-match",
            etag,
            "--status",
            "archived",
            "--archived-in-commit-json",
            r#"{"commit":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}"#,
        ],
    );
    directory
}

// Implementation aid: this checks archive visibility across public CLI reads.
#[test]
fn archived_requirement_is_hidden_from_default_reads_and_explicit_reads_show_it() {
    let directory = archived_repository();
    let repo = directory.path().to_str().unwrap();
    assert_eq!(
        listed_ids(&run(repo, &["requirements", "list", "--format", "json"])),
        ["req_current"]
    );
    assert_eq!(
        listed_ids(&run(
            repo,
            &[
                "requirements",
                "list",
                "--exclude-terminal",
                "false",
                "--format",
                "json"
            ]
        )),
        ["req_current", "req_retired"]
    );
    let document = run(
        repo,
        &["req_current", "--view", "review", "--format", "json"],
    );
    assert!(!document.contains("req_retired"), "{document}");
    assert!(run(
        repo,
        &[
            "api",
            "requirements/req_current/document",
            "--query",
            "exclude_terminal=false",
            "--format",
            "json"
        ]
    )
    .contains("req_retired"));
    let archived: serde_json::Value = serde_json::from_str(&run(
        repo,
        &["api", "requirements/req_retired", "--format", "json"],
    ))
    .unwrap();
    assert_eq!(archived["data"]["status"], "archived");
    assert_eq!(
        archived["data"]["archived_in_commit"]["commit"],
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    );
}
