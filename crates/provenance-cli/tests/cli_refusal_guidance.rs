use assert_cmd::Command;
mod support {
    pub mod review_paths;
}
use support::review_paths::{init, json_output, create_requirement, provenance};

fn json(repo: &str, args: &[&str]) -> serde_json::Value {
    json_output(provenance().args(args).args(["--repo", repo]))
}

fn run(repo: &str, args: &[&str]) -> std::process::Output {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
        .args(args)
        .args(["--repo", repo])
        .output()
        .unwrap()
}

// Implementation aids: public CLI refusals must name a usable recovery command.
#[test]
fn create_help_excludes_options_from_other_actions_and_types() {
    let output = run(
        "/no-repository",
        &["req_help", "create", "--type", "requirement", "--help"],
    );
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).unwrap();
    assert!(help.contains("--statement"));
    for flag in ["--source-ids", "--source-ref", "--if-match", "--view", "--position", "--made-by"] {
        assert!(!help.contains(flag), "{help}");
    }
}

#[test]
fn empty_search_names_text_and_kind_forms() {
    let (_dir, repo) = init();
    let repo = repo.as_str();
    let output = run(repo, &["search"]);
    assert_eq!(output.status.code(), Some(1));
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(error.contains("search requires text or at least one filter"), "{error}");
    assert!(
        error.contains("provenance search --text <query>"),
        "{error}"
    );
    assert!(error.contains("provenance search --kind <kind>"), "{error}");
    assert!(run(repo, &["search", "--kind", "requirement"])
        .status
        .success());
}

#[test]
fn ownership_refusal_names_the_manual_owner_and_handoff() {
    let (_dir, repo) = init();
    let repo = repo.as_str();
    let before = create_requirement(repo, "req_owner");
    let output = run(
        repo,
        &[
            "req_owner",
            "update",
            "--declared-by",
            "other",
            "--if-match",
            before["data"]["edit"]["etag"].as_str().unwrap(),
            "--statement",
            "The owner controls changes.",
        ],
    );
    assert_eq!(output.status.code(), Some(1));
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(error.contains("record_ownership_conflict"), "{error}");
    assert!(error.contains("manual record"), "{error}");
    assert!(error.contains("omit --declared-by"), "{error}");
    assert!(!error.contains("ask the owner"), "{error}");
    let after = json(
        repo,
        &["requirements", "req_owner", "get", "--format", "json"],
    );
    assert_eq!(after["data"]["edit"], before["data"]["edit"]);
}

#[test]
fn submit_after_guarded_update_names_the_pending_review_and_withdrawal() {
    let (_dir, repo) = init();
    let repo = repo.as_str();
    let before = create_requirement(repo, "req_pending");
    let updated = json(
        repo,
        &[
            "req_pending",
            "update",
            "--if-match",
            before["data"]["edit"]["etag"].as_str().unwrap(),
            "--statement",
            "The record keeps its review state.",
            "--format",
            "json",
        ],
    );
    let pending = updated["data"]["decision"]["pending"]["proposal_id"]
        .as_str()
        .unwrap();
    let output = run(
        repo,
        &[
            "req_pending",
            "submit",
            "--actor",
            "agent",
            "--title",
            "Review",
            "--summary",
            "Review the record.",
            "--source-ids-json", "[]", "--evidence-references-json", "[]", "--builds-on-json", "[]",
        ],
    );
    assert_eq!(output.status.code(), Some(1));
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(
        error.contains(&format!("already has pending submission {pending}")),
        "{error}"
    );
    assert!(
        error.contains("provenance req_pending --view review"),
        "{error}"
    );
    let withdraw = format!(
        "provenance requirements req_pending submissions {pending} withdraw --actor <actor>"
    );
    assert!(error.contains(&withdraw), "{error}");
    let after = json(
        repo,
        &["requirements", "req_pending", "get", "--format", "json"],
    );
    assert_eq!(after["data"]["decision"], updated["data"]["decision"]);
    json(
        repo,
        &[
            "requirements",
            "req_pending",
            "submissions",
            pending,
            "withdraw",
            "--actor",
            "agent",
            "--format",
            "json",
        ],
    );
}

#[test]
fn ownership_refusal_names_the_declared_owner() {
    let (_dir, repo) = init();
    let repo = repo.as_str();
    json(
        repo,
        &[
            "req_parent",
            "create",
            "--type",
            "requirement",
            "--statement",
            "The decision has a parent.",
            "--format",
            "json",
        ],
    );
    json(
        repo,
        &[
            "res_owned",
            "create",
            "--type",
            "resolution",
            "--requirement-id",
            "req_parent",
            "--title",
            "Owner",
            "--position",
            "Keep the owner.",
            "--rationale",
            "The owner controls changes.",
            "--made-by",
            "spec://owner",
            "--format",
            "json",
        ],
    );
    let output = run(
        repo,
        &[
            "res_owned",
            "submit",
            "--actor",
            "agent",
            "--declared-by",
            "other",
            "--title",
            "Review",
            "--summary",
            "Review the decision.",
            "--source-ids-json",
            "[]",
            "--evidence-references-json",
            "[]",
            "--builds-on-json",
            "[]",
        ],
    );
    assert_eq!(output.status.code(), Some(1));
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(error.contains("record_ownership_conflict"), "{error}");
    assert!(error.contains("spec://owner"), "{error}");
    assert!(error.contains("--declared-by"), "{error}");
    assert!(error.contains("ask the owner"), "{error}");
}

#[test]
fn generic_create_help_asks_for_a_type_before_listing_record_fields() {
    let output = run("/no-repository", &["req_help", "create", "--help"]);
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).unwrap();
    assert!(
        !help.contains("--source-ref") && !help.contains("--source-ids-json") && !help.contains("--statement"),
        "{help}"
    );
    assert!(
        help.contains("provenance <id> create --type <kind> --help"),
        "{help}"
    );
}

#[test]
fn help_exits_before_other_input_validation() {
    for args in [vec!["req_x", "--follow", "--help"], vec!["req_x", "create", "--type", "bogus", "--help"]] {
        let output = run("/no-repository", &args);
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        let help = String::from_utf8(output.stdout).unwrap();
        assert!(help.contains("Print help"), "{help}");
    }
}

#[test]
fn submit_names_every_missing_evidence_list_before_pending_conflict() {
    let (_dir, repo) = init();
    create_requirement(&repo, "req_missing");
    for supplied in 0..3 {
        let mut args = vec!["req_missing", "submit", "--actor", "agent", "--title", "Review", "--summary", "Review the record."];
        let fields = ["--source-ids-json", "--evidence-references-json", "--builds-on-json"];
        for field in &fields[..supplied] { args.extend([field, "[]"]); }
        let output = run(&repo, &args);
        assert_eq!(output.status.code(), Some(1));
        let error = String::from_utf8(output.stderr).unwrap();
        assert!(error.contains("invalid_input"), "{error}");
        assert!(!error.contains("review_submission_conflict"), "{error}");
        for field in &fields[supplied..] { assert!(error.contains(&format!("{field} '[]'")), "{error}"); }
    }
}
