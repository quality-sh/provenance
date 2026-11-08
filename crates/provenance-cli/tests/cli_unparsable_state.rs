//! A state file line that does not parse is a classified refusal. The CLI
//! answer names the file below the state directory, the line number and the
//! parse message, for a write, an `api` read and a `get`.

use provenance_macros::verifies;
use serde_json::Value;

mod support {
    pub mod review_paths;
}
use support::review_paths::{create_requirement, provenance};

const CARDS: &str = "scopes/default/ideation/proposal_cards.jsonl";

/// A repository with one requirement whose first proposal card names an
/// artifact type that this build does not know.
fn unknown_artifact_type() -> (tempfile::TempDir, String) {
    let (directory, repo) = support::review_paths::init();
    create_requirement(&repo, "req_a");
    let path = directory.path().join(".provenance/state").join(CARDS);
    let text = std::fs::read_to_string(&path).unwrap();
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    let mut card: Value = serde_json::from_str(&lines[0]).unwrap();
    card["traceability"]["target"]["artifact_type"] = "widget".into();
    lines[0] = card.to_string();
    std::fs::write(&path, lines.join("\n") + "\n").unwrap();
    (directory, repo)
}

/// The refusal object on standard error, with or without the `Error: ` prefix.
fn refusal(repo: &str, args: &[&str]) -> Value {
    let output = provenance()
        .args(args)
        .args(["--repo", repo])
        .output()
        .unwrap();
    let text = String::from_utf8(output.stderr).unwrap();
    assert_eq!(output.status.code(), Some(1), "{args:?}: {text}");
    let line = text.lines().next().unwrap_or_default();
    let object = line.strip_prefix("Error: ").unwrap_or(line);
    serde_json::from_str(object).unwrap_or_else(|error| panic!("{args:?}: {error}: {text}"))
}

fn assert_names_the_line(refusal: &Value) {
    let error = &refusal["error"];
    assert_eq!(error["kind"], "invalid_state_file", "{refusal}");
    assert_eq!(error["file"], CARDS, "{refusal}");
    assert_eq!(error["line"], 1, "{refusal}");
    let message = error["message"].as_str().unwrap_or_default();
    assert!(message.starts_with("unknown variant `widget`"), "{refusal}");
    assert!(!refusal.to_string().contains(".provenance"), "{refusal}");
}

#[test]
#[verifies("rule_state_parse_refusal_names_the_line", examples)]
fn a_create_names_the_state_line_that_does_not_parse() {
    let (_directory, repo) = unknown_artifact_type();
    let refusal = refusal(
        &repo,
        &[
            "req_b",
            "create",
            "--type",
            "requirement",
            "--statement",
            "The system keeps review feedback available.",
            "--format",
            "json",
        ],
    );
    assert_names_the_line(&refusal);
}

#[test]
#[verifies("rule_state_parse_refusal_names_the_line", examples)]
fn an_api_read_names_the_state_line_that_does_not_parse() {
    let (_directory, repo) = unknown_artifact_type();
    let refusal = refusal(&repo, &["api", "/requirements/req_a"]);
    assert_names_the_line(&refusal);
}

#[test]
#[verifies("rule_state_parse_refusal_names_the_line", examples)]
fn a_get_names_the_state_line_that_does_not_parse() {
    let (_directory, repo) = unknown_artifact_type();
    let refusal = refusal(&repo, &["req_a", "get", "--format", "json"]);
    assert_names_the_line(&refusal);
}
