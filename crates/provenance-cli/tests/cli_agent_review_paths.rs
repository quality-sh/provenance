use assert_cmd::Command;
use predicates::str::contains;
use serde_json::{json, Value};

fn provenance() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
}

fn init() -> (tempfile::TempDir, String) {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_string_lossy().into_owned();
    provenance()
        .args([
            "init",
            "--path",
            &repo,
            "--scope",
            "default",
            "--path-prefix",
            ".",
            "--disposition-actor-id",
            "reviewer",
        ])
        .assert()
        .success();
    (directory, repo)
}

fn json_output(command: &mut Command) -> Value {
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn create_requirement(repo: &str, id: &str) -> Value {
    json_output(provenance().args([
        "requirements",
        "create",
        "--repo",
        repo,
        "--id",
        id,
        "--statement",
        "The system keeps review feedback available.",
        "--format",
        "json",
    ]))
}

fn reject_pending(repo: &str, id: &str, created: &Value) {
    let proposal = created["data"]["decision"]["pending"]["proposal_id"]
        .as_str()
        .unwrap();
    provenance()
        .args([
            "requirements",
            id,
            "submissions",
            proposal,
            "decide",
            "--repo",
            repo,
            "--stdin",
        ])
        .write_stdin(
            json!({
                "actor": {"identity_type":"human", "id":"reviewer"},
                "decision": "rejected",
                "rationale": "The statement needs one precise condition.",
                "canonical_artifact": null,
                "feedback": {"role":"user", "body":"Name the condition before resubmission."},
                "declared_by": null
            })
            .to_string(),
        )
        .assert()
        .success();
}

#[test]
fn review_view_returns_decision_comment_discussions_and_update_precondition() {
    let (_directory, repo) = init();
    let created = create_requirement(&repo, "req_feedback");
    reject_pending(&repo, "req_feedback", &created);

    let review = json_output(provenance().args([
        "req_feedback",
        "get",
        "--view",
        "review",
        "--repo",
        &repo,
        "--format",
        "json",
    ]));

    assert_eq!(review["view"], "review");
    assert!(review["review"]["edit"]["etag"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));
    assert_eq!(
        review["review"]["update_precondition"],
        format!(
            "--if-match {}",
            review["review"]["edit"]["etag"].as_str().unwrap()
        )
    );
    assert_eq!(
        review["review"]["decision"]["decisions"][0]["disposition"]["decision"],
        "rejected"
    );
    assert_eq!(
        review["review"]["discussions"]["entries"][0]["messages"]["entries"][0]["body"],
        "Name the condition before resubmission."
    );
}

#[test]
fn wrong_feedback_form_names_the_review_command() {
    let (_directory, repo) = init();
    create_requirement(&repo, "req_wrong_form");

    provenance()
        .args([
            "requirements",
            "req_wrong_form",
            "feedback",
            "--repo",
            &repo,
        ])
        .assert()
        .failure()
        .stderr(contains("provenance req_wrong_form get --view review"));
}

#[test]
fn explicit_submit_explains_that_the_automatic_submission_is_pending() {
    let (_directory, repo) = init();
    let created = create_requirement(&repo, "req_pending");
    let proposal = created["data"]["decision"]["pending"]["proposal_id"]
        .as_str()
        .unwrap();
    let revision = created["data"]["edit"]["revision"].as_str().unwrap();

    provenance()
        .args(["req_pending", "submit", "--repo", &repo, "--stdin"])
        .write_stdin(
            json!({
                "actor":"agent", "declared_by":null, "title":"Review",
                "summary":"Review the current record.", "confidence":null,
                "source_ids":[], "evidence_references":[], "builds_on":[],
                "expected_revision":revision, "revises":null
            })
            .to_string(),
        )
        .assert()
        .failure()
        .stderr(contains(format!(
            "already has pending submission {proposal}"
        )))
        .stderr(contains("provenance req_pending get --view review"));
}

#[test]
fn api_catalog_is_bounded_by_default_and_explains_filtering() {
    let (_directory, repo) = init();
    let output = provenance()
        .args(["api", "--repo", &repo])
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.lines().count() < 500, "{} lines", text.lines().count());
    assert!(text.contains("Use --filter <text> or --limit <number> to see more."));

    provenance()
        .args(["api", "--repo", &repo, "--filter", "requirements/{id}"])
        .assert()
        .success()
        .stdout(contains("/requirements/{id}"));
}

#[test]
fn update_help_and_invalid_if_match_name_the_exact_input() {
    let (_directory, repo) = init();
    create_requirement(&repo, "req_etag");

    provenance()
        .args(["requirements", "req_etag", "update", "--help"])
        .assert()
        .success()
        .stdout(contains("data.edit.etag"))
        .stdout(contains("--if-match"));

    provenance()
        .args([
            "requirements",
            "req_etag",
            "update",
            "--repo",
            &repo,
            "--if-match",
            "revision_1",
            "--description",
            "Changed",
        ])
        .assert()
        .failure()
        .stderr(contains("--if-match must equal data.edit.etag"))
        .stderr(contains("sha256:<64 lowercase hexadecimal characters>"));
}

#[test]
fn terminal_records_are_hidden_by_default_and_have_an_explicit_opt_in() {
    let (_directory, repo) = init();
    let requirement = create_requirement(&repo, "req_terminal");
    let etag = requirement["data"]["edit"]["etag"].as_str().unwrap();
    provenance()
        .args([
            "rules",
            "create",
            "--repo",
            &repo,
            "--id",
            "rule_archived",
            "--statement",
            "The system keeps terminal records available on request.",
            "--requirement-id",
            "req_terminal",
            "--status",
            "archived",
            "--archived-in-commit-json",
            &json!({"commit":"a".repeat(40),"at":null}).to_string(),
        ])
        .assert()
        .success();

    let listed =
        json_output(provenance().args(["rules", "list", "--repo", &repo, "--format", "json"]));
    assert!(listed["data"]["items"].as_array().unwrap().is_empty());

    let included = json_output(provenance().args([
        "rules",
        "list",
        "--repo",
        &repo,
        "--exclude-terminal",
        "false",
        "--format",
        "json",
    ]));
    assert_eq!(included["data"]["items"][0]["id"], "rule_archived");

    let searched = json_output(provenance().args([
        "search",
        "--repo",
        &repo,
        "--text",
        "terminal records",
        "--format",
        "json",
    ]));
    assert!(searched["result"]["nodes"].as_array().unwrap().is_empty());
    let searched = json_output(provenance().args([
        "search",
        "--repo",
        &repo,
        "--text",
        "terminal records",
        "--include-terminal",
        "--format",
        "json",
    ]));
    assert_eq!(searched["result"]["nodes"][0]["id"], "rule_archived");

    let _ = etag;
}

#[test]
fn installed_guidance_documents_feedback_and_rejected_revision_paths() {
    let (directory, _repo) = init();
    let agents = std::fs::read_to_string(directory.path().join("AGENTS.md")).unwrap();
    let skills = std::fs::read_dir(directory.path().join(".agents/skills"))
        .unwrap()
        .map(|entry| std::fs::read_to_string(entry.unwrap().path().join("SKILL.md")).unwrap())
        .collect::<Vec<_>>()
        .join("\n");
    let guidance = format!("{agents}\n{skills}");

    assert!(guidance.contains("provenance <record-id> get --view review"));
    assert!(guidance.contains("A guarded update after a rejection opens the new submission"));
    assert!(guidance.contains("data.edit.etag"));
}
