#[path = "shaping_support/fixtures.rs"]
mod fixtures;
#[path = "shaping_support/provenance.rs"]
mod provenance;

use fixtures::{create_source_and_requirement, create_topic, init};
use provenance::{provenance, provenance_stdin};
use provenance_macros::verifies;
use serde_json::{json, Value};

fn setup() -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo").to_string_lossy().into_owned();
    init(&repo);
    create_source_and_requirement(&repo);
    create_topic(&repo);
    (dir, repo)
}

fn create(repo: &str, id: &str, dependencies: Option<Value>) -> Value {
    let mut input = json!({"id":id,"topic_id":"topic_overtime",
        "question":"Which threshold applies?","resolution_method":"research"});
    if let Some(dependencies) = dependencies {
        input["depends_on"] = dependencies;
    }
    let output = provenance_stdin(&["questions", "create", "--repo", repo,
        "--scope", "default", "--stdin", "--format", "json"], &input.to_string())
        .success().get_output().stdout.clone();
    serde_json::from_slice(&output).unwrap()
}

fn summary(repo: &str, options: &[&str]) -> Value {
    let mut args = vec!["topic_overtime", "get", "--repo", repo,
        "--scope", "default", "--view", "children", "--format", "json"];
    args.extend_from_slice(options);
    let output = provenance(&args).success().get_output().stdout.clone();
    let value: Value = serde_json::from_slice(&output).unwrap();
    assert!(value["shaping"].is_object(), "missing shaping summary: {value}");
    value["shaping"].clone()
}

#[test]
#[verifies("rule_shaping_question_ready", conformance)]
fn readiness_matches_answered_prerequisite_oracle_after_restart() {
    let (_dir, repo) = setup();
    create(&repo, "question_a", None);
    create(&repo, "question_b", None);
    create(&repo, "question_c", Some(json!(["question_b", "question_a", "question_b"])));
    let state = summary(&repo, &[]);
    assert_eq!(state["ready_ids"], json!(["question_a", "question_b"]));
    assert_eq!(state["complete"], true);
    provenance(&["questions", "question_a", "answer", "--repo", &repo,
        "--scope", "default", "--answer", "Use the source threshold."]).success();
    assert_eq!(summary(&repo, &[])["ready_ids"], json!(["question_b"]));
    provenance(&["questions", "question_b", "answer", "--repo", &repo,
        "--scope", "default", "--answer", "Use the daily threshold."]).success();
    assert_eq!(summary(&repo, &[])["ready_ids"], json!(["question_c"]));
}

#[test]
#[verifies("rule_shaping_recovery_bounds", examples)]
fn bounded_children_cannot_authorize_selection() {
    let (_dir, repo) = setup();
    create(&repo, "question_a", None);
    create(&repo, "question_b", None);
    assert_eq!(summary(&repo, &["--limit", "1"])["complete"], false);
    assert_eq!(summary(&repo, &["--returned-kind", "question"])["complete"], false);
}
