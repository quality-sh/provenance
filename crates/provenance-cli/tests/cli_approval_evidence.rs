use assert_cmd::Command;
use serde_json::{json, Value};

fn cli() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
}

fn run(repo: &str, args: &[&str], input: Option<Value>) -> std::process::Output {
    let mut command = cli();
    command.args(args).args(["--repo", repo, "--format", "json"]);
    if let Some(input) = input {
        command.arg("--stdin").write_stdin(input.to_string());
    }
    command.output().unwrap()
}

fn success(output: std::process::Output) -> Value {
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    serde_json::from_slice(&output.stdout).unwrap()
}

// Implementation aid: the bead requires a public diagnostic and an unchanged pending submission.
#[test]
fn incomplete_approval_reports_missing_evidence_and_keeps_submission_pending() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap();
    cli().args(["init", "--path", repo, "--scope", "default", "--path-prefix", ".",
        "--disposition-actor-id", "reviewer"]).assert().success();
    let created = success(run(repo, &["requirements", "create", "--id", "req_review",
        "--statement", "The system stores each result."], None));
    let proposal = created["data"]["decision"]["pending"]["proposal_id"].as_str().unwrap();
    let decision = json!({"actor":{"identity_type":"human","id":"reviewer"},"decision":"accepted"});
    let before = success(run(repo, &["requirements", "req_review", "get"], None));
    let refused = run(repo, &["requirements", "req_review", "submissions", proposal, "decide"], Some(decision.clone()));
    eprintln!("Before/After incomplete approval: {}", String::from_utf8_lossy(&refused.stderr));
    assert!(!refused.status.success());
    let error: Value = serde_json::from_slice(&refused.stderr).unwrap();
    assert_eq!(error["error"]["kind"], "approval_evidence_required", "{error}");
    assert_eq!(error["error"]["message"], "An approval requires a prior assertion or a human canonical_artifact.");
    assert_eq!(success(run(repo, &["requirements", "req_review", "get"], None)), before);
    let mut complete = decision;
    complete["canonical_artifact"] = json!({"artifact_type":"requirement","artifact_id":"req_review"});
    success(run(repo, &["requirements", "req_review", "submissions", proposal, "decide"], Some(complete)));
    let after = success(run(repo, &["requirements", "req_review", "get"], None));
    assert!(after["data"]["decision"]["pending"].is_null());
    assert_eq!(after["data"]["decision"]["decisions"][0]["disposition"]["decision"], "accepted");
}
