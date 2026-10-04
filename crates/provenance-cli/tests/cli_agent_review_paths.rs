use assert_cmd::Command;
use predicates::{prelude::PredicateBooleanExt as _, str::contains};
use provenance_macros::verifies;
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
#[verifies("rule_porcelain_review_returns_current_state", examples)]
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
#[verifies("rule_review_refusal_names_read_command", examples)]
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
#[verifies("rule_review_refusal_names_read_command", examples)]
fn explicit_submit_explains_that_the_automatic_submission_is_pending() {
    let (_directory, repo) = init();
    let created = create_requirement(&repo, "req_pending");
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
        .stderr(contains("provenance req_pending get --view review"));
}

#[test]
#[verifies("rule_cli_guard_guidance", examples)]
fn invalid_if_match_names_the_edit_etag_form() {
    let (_directory, repo) = init();
    create_requirement(&repo, "req_etag");

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
#[verifies("rule_porcelain_output_reports_bounds", examples)]
/// Covers the review flow when Discussion and message pages are both truncated.
fn review_view_gives_commands_for_each_truncated_feedback_page() {
    let (_directory, repo) = init();
    create_requirement(&repo, "req_bounded_feedback");
    for number in 1..=2 {
        let started = json_output(provenance().args([
            "req_bounded_feedback",
            "discuss",
            "--repo",
            &repo,
            "--body",
            &format!("Opening {number}"),
            "--format",
            "json",
        ]));
        let discussion_id = started["receipt"]["discussion_id"].as_str().unwrap();
        json_output(provenance().args([
            discussion_id,
            "reply",
            "--repo",
            &repo,
            "--body",
            &format!("Reply {number}"),
            "--expected-version",
            "1",
            "--format",
            "json",
        ]));
    }

    let review = json_output(provenance().args([
        "req_bounded_feedback",
        "get",
        "--view",
        "review",
        "--limit",
        "1",
        "--repo",
        &repo,
        "--format",
        "json",
    ]));
    let commands = review["review"]["follow_up_commands"]
        .as_array()
        .unwrap()
        .iter()
        .map(|command| command.as_str().unwrap())
        .collect::<Vec<_>>();
    let shown_id = review["review"]["discussions"]["entries"][0]["head"]["discussion_id"]
        .as_str()
        .unwrap();
    assert!(commands.iter().any(|command| command
        .starts_with("provenance req_bounded_feedback discussions --limit 1 --cursor ")));
    assert!(commands.iter().any(|command| command.starts_with(&format!(
        "provenance discussions {shown_id} get --limit 1 --cursor "
    ))));
}

fn repo_with_archived_rule() -> (tempfile::TempDir, String) {
    let (directory, repo) = init();
    create_requirement(&repo, "req_terminal");
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
    (directory, repo)
}

#[test]
#[verifies("rule_review_defaults_exclude_terminal_records", examples)]
fn collection_hides_terminal_records_by_default() {
    let (_directory, repo) = repo_with_archived_rule();

    let listed =
        json_output(provenance().args(["rules", "list", "--repo", &repo, "--format", "json"]));
    assert_eq!(listed["data"]["items"], serde_json::json!([]));
}

#[test]
#[verifies("rule_review_defaults_exclude_terminal_records", examples)]
fn named_query_hides_terminal_records_by_default() {
    let (_directory, repo) = repo_with_archived_rule();

    let searched = json_output(provenance().args([
        "rules",
        "search",
        "--repo",
        &repo,
        "--text",
        "terminal records",
        "--format",
        "json",
    ]));
    assert_eq!(searched["data"]["items"], serde_json::json!([]));
}

#[test]
#[verifies("rule_cli_terminal_records_opt_in", examples)]
fn collection_includes_terminal_records_on_request() {
    let (_directory, repo) = repo_with_archived_rule();
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
}

#[test]
#[verifies("rule_review_defaults_exclude_terminal_records", examples)]
fn search_hides_terminal_records_by_default() {
    let (_directory, repo) = repo_with_archived_rule();
    let searched = json_output(provenance().args([
        "search",
        "--repo",
        &repo,
        "--text",
        "terminal records",
        "--format",
        "json",
    ]));
    assert_eq!(searched["nodes"], serde_json::json!([]));
}

#[test]
#[verifies("rule_cli_terminal_records_opt_in", examples)]
fn search_includes_terminal_records_on_request() {
    let (_directory, repo) = repo_with_archived_rule();
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
    assert_eq!(searched["nodes"][0]["id"], "rule_archived");
}
