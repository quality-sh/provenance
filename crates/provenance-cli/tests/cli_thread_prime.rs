use assert_cmd::Command;
use provenance_core::SUPPORTED_SCHEMA_VERSION;

#[test]
fn cli_thread_prime_posts_message_and_renders_context() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().to_string_lossy().to_string();

    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "init",
            "--path",
            &repo,
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "requirements",
            "create",
            "--repo",
            &repo,
            "--scope",
            "default",
            "--id",
            "req_schads_overtime",
            "--statement",
            "Overtime must follow SCHADS thresholds",
        ])
        .assert()
        .success();
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "rules",
            "create",
            "--repo",
            &repo,
            "--scope",
            "default",
            "--id",
            "rule_schads_pay_001",
            "--requirement-id",
            "req_schads_overtime",
            "--statement",
            "Pay overtime after the threshold",
        ])
        .assert()
        .success();
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "thread",
            "post",
            "--repo",
            &repo,
            "--scope",
            "default",
            "--parent-type",
            "rule",
            "--parent-id",
            "rule_schads_pay_001",
            "--role",
            "assistant",
            "Traceability verified locally",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("thread_rule_rule_schads_pay_001"))
        .stdout(predicates::str::contains("msg_000001"));
    Command::cargo_bin("provenance")
        .unwrap()
        .args(["materialize", "--repo", &repo, "--format", "json"])
        .assert()
        .success();
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "prime",
            "--repo",
            &repo,
            "--scope",
            "default",
            "--format",
            "markdown",
            "--include-threads",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("rule_schads_pay_001"))
        .stdout(predicates::str::contains("thread_rule_rule_schads_pay_001"))
        .stdout(predicates::str::contains("msg_000001"));
}

#[test]
fn cli_thread_list_preserves_resolved_status() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().to_string_lossy().to_string();

    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "init",
            "--path",
            &repo,
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();

    let threads_dir = dir.path().join(".provenance/state/scopes/default/threads");
    std::fs::create_dir_all(&threads_dir).unwrap();
    std::fs::write(
        threads_dir.join("threads.jsonl"),
        serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0,"scope_id":"default","id":"thread_rule_rule_sah_001","parent":{"node_type":"rule","node_id":"rule_sah_001"},"status":"resolved","created_at":1}).to_string(),
    )
    .unwrap();

    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "thread", "list", "--repo", &repo, "--scope", "default", "--format", "json",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains(r#""status": "resolved""#));
}
