use provenance_macros::verifies;
mod support {
    pub mod review_paths;
}
use support::review_paths::{create_requirement, init, json_output, provenance};
#[path = "support/bounded_process.rs"]
mod bounded_process;
use bounded_process::Bounded;

fn json(repo: &str, args: &[&str]) -> serde_json::Value {
    json_output(provenance().args(args).args(["--repo", repo]))
}

fn run(repo: &str, args: &[&str]) -> std::process::Output {
    provenance()
        .args(args)
        .args(["--repo", repo])
        .output()
        .unwrap()
}

#[test]
#[verifies("rule_create_help_accepted_fields", examples)]
fn create_help_excludes_options_from_other_actions_and_types() {
    let output = run(
        "/no-repository",
        &["req_help", "create", "--type", "requirement", "--help"],
    );
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).unwrap();
    assert!(help.contains("--statement"));
    for flag in [
        "--source-ids",
        "--source-ref",
        "--if-match",
        "--view",
        "--position",
        "--made-by",
    ] {
        assert!(!help.contains(flag), "{help}");
    }
}

#[test]
#[verifies("rule_search_refusal_names_query", examples)]
fn empty_search_names_text_and_kind_forms() {
    let (_dir, repo) = init();
    let repo = repo.as_str();
    let output = run(repo, &["search"]);
    assert_eq!(output.status.code(), Some(1));
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(
        error.contains("search requires text or at least one filter"),
        "{error}"
    );
    assert!(
        error.contains("provenance search --text <query>"),
        "{error}"
    );
    assert!(error.contains("provenance search --kind <kind>"), "{error}");
    assert!(run(repo, &["search", "--kind", "requirement"])
        .status
        .success());
}

/// Implementation aid: no Rule states the refusal of search text without `--text`, and
/// `rule_search_refusal_names_query` is about a search with no query.
#[test]
fn search_words_without_text_flag_name_the_text_flag() {
    let cases: [(&[&str], &str, Option<&str>); 15] = [
        (
            &["search", "some text"],
            "some text",
            Some("--text 'some text'"),
        ),
        (
            &["search", "it's here"],
            "it's here",
            Some(r"--text 'it'\''s here'"),
        ),
        (
            &["search", "some text", "--kind", "requirement"],
            "some text",
            Some("--text 'some text' --kind requirement"),
        ),
        (
            &["search", "some", "text"],
            "some",
            Some("--text 'some text'"),
        ),
        (
            &["search", "--text", "some", "text"],
            "text",
            Some("--text 'some text'"),
        ),
        (&["search", "--", "x"], "x", Some("--text 'x'")),
        (&["search", "--", "-x"], "-x", Some("--text '-x'")),
        (
            &["--repo", "r", "search", "x"],
            "x",
            Some("--text 'x' --repo r"),
        ),
        (&["search", "x", "--help"], "x", Some("--text 'x'")),
        (&["search", "x", "-h"], "x", Some("--text 'x'")),
        (&["help", "search", "x"], "x", Some("--text 'x'")),
        (&["search", ""], "", None),
        (&["search", "x", "--bogus", "v"], "x", None),
        (&["search", "x", "--kind", "--limit", "5"], "x", None),
        (&["search", "x", "-y"], "x", None),
    ];
    for (words, found, corrected) in cases {
        let tip = corrected
            .map(|corrected| format!(": provenance search {corrected}"))
            .unwrap_or_default();
        assert_refusal(
            words,
            &format!(
                "error: unexpected argument '{found}' found\n\n  \
                 tip: give the search text with --text{tip}\n\n\
                 For more information, try 'provenance search --help'.\n"
            ),
        );
    }
    assert_refusal(
        &["search", "-x"],
        "error: unexpected argument '-x' found\n\n  \
         tip: to pass '-x' as a value, use '-- -x'\n\n\
         Usage: provenance [OPTIONS] <COMMAND>\n\n\
         For more information, try '--help'.\n",
    );
}

fn assert_refusal(words: &[&str], error: &str) {
    let output = bounded_process::provenance().args(words).bounded_output();
    assert_eq!(output.status.code(), Some(2), "{words:?} {output:?}");
    assert!(output.stdout.is_empty(), "{words:?} {output:?}");
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        error,
        "{words:?}"
    );
}

#[test]
#[verifies("rule_ownership_refusal_names_owner", examples)]
fn ownership_refusal_for_a_manual_record_omits_owner_handoff() {
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
    assert!(!error.contains("no declared_by owner"), "{error}");
    let after = json(repo, &["api", "requirements/req_owner", "--format", "json"]);
    assert_eq!(after["data"]["edit"], before["data"]["edit"]);
}

#[test]
#[verifies("rule_review_refusal_names_read_command", examples)]
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
    assert!(
        error.contains(&format!("already has pending submission {pending}")),
        "{error}"
    );
    assert!(
        error.contains("provenance req_pending --view review"),
        "{error}"
    );
    let withdraw = format!(
        "provenance api requirements/req_pending/submissions/{pending}/withdraw --method post --input -"
    );
    assert!(error.contains(&withdraw), "{error}");
    assert!(error.contains(r#"'{"actor":"<actor>"}'"#), "{error}");
    let after = json(
        repo,
        &["api", "requirements/req_pending", "--format", "json"],
    );
    assert_eq!(after["data"]["decision"], updated["data"]["decision"]);
    let body = std::path::Path::new(repo).join("withdraw.json");
    std::fs::write(&body, r#"{"actor":"agent"}"#).unwrap();
    json(
        repo,
        &[
            "api",
            &format!("requirements/req_pending/submissions/{pending}/withdraw"),
            "--method",
            "post",
            "--input",
            body.to_str().unwrap(),
            "--format",
            "json",
        ],
    );
}

#[test]
#[verifies("rule_ownership_refusal_names_owner", examples)]
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
#[verifies("rule_create_help_accepted_fields", examples)]
fn generic_create_help_asks_for_a_type_before_listing_record_fields() {
    let output = run("/no-repository", &["req_help", "create", "--help"]);
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).unwrap();
    assert!(
        !help.contains("--source-ref")
            && !help.contains("--source-ids-json")
            && !help.contains("--statement"),
        "{help}"
    );
    assert!(
        help.contains("provenance <id> create --type <kind> --help"),
        "{help}"
    );
}

#[test]
#[provenance_macros::verifies("rule_record_help_exits_before_validation", examples)]
fn help_exits_before_other_input_validation() {
    for args in [
        vec!["req_x", "--follow", "--help"],
        vec!["req_x", "create", "--type", "bogus", "--help"],
    ] {
        let output = run("/no-repository", &args);
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        let help = String::from_utf8(output.stdout).unwrap();
        assert!(help.contains("Print help"), "{help}");
        if args.contains(&"create") {
            assert!(
                help.contains("provenance <id> create --type <kind> --help"),
                "{help}"
            );
            assert!(!help.contains("--statement"), "{help}");
        } else {
            assert!(help.contains("--view"), "{help}");
            assert!(help.contains("--follow"), "{help}");
            assert!(!help.contains("Select a record type"), "{help}");
        }
    }
}

#[test]
#[provenance_macros::verifies("rule_submit_refusal_names_missing_lists", examples)]
fn submit_names_every_missing_evidence_list_before_pending_conflict() {
    let (_dir, repo) = init();
    create_requirement(&repo, "req_missing");
    for supplied in 0..3 {
        let mut args = vec![
            "req_missing",
            "submit",
            "--actor",
            "agent",
            "--title",
            "Review",
            "--summary",
            "Review the record.",
        ];
        let fields = [
            "--source-ids-json",
            "--evidence-references-json",
            "--builds-on-json",
        ];
        for field in &fields[..supplied] {
            args.extend([field, "[]"]);
        }
        let output = run(&repo, &args);
        assert_eq!(output.status.code(), Some(1));
        let error = String::from_utf8(output.stderr).unwrap();
        assert!(error.contains("invalid_input"), "{error}");
        assert!(!error.contains("review_submission_conflict"), "{error}");
        for field in &fields[..supplied] {
            assert!(!error.contains(field), "{error}");
        }
        for field in &fields[supplied..] {
            assert!(error.contains(&format!("{field} '[]'")), "{error}");
        }
    }
}
