//! provenance-0str: the plan and the apply of one typed document that deletes
//! a Requirement, for each state of the records that the deletion removes.
//! Each case prints the full plan answer and the full apply answer.

use crate::porcelain_authoring_support::{json_output, json_stdin_output, provenance};
use serde_json::{json, Value};

const OWNER: &str = "spec://typescript/share-links";

fn document(requirements: &Value) -> Value {
    json!({
        "spec": "share-links",
        "declared_by": OWNER,
        "sources": [{"key": "policy", "name": "Policy", "kind": "policy"}],
        "requirements": requirements,
        "rules": []
    })
}

fn both(statement: &str) -> Value {
    json!([
        {"key": "sharing", "statement": statement, "sources": ["policy"]},
        {"key": "other", "statement": "The host starts."}
    ])
}

fn after() -> Value {
    document(&json!([{"key": "other", "statement": "The host starts."}]))
}

/// An initialized repository whose reviewer is `reviewer`.
fn initialized_repo() -> (tempfile::TempDir, String) {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_string_lossy().into_owned();
    provenance()
        .args([
            "init", "--path", &repo, "--scope", "default", "--path-prefix", ".",
            "--disposition-actor-id", "reviewer",
        ])
        .assert()
        .success();
    (directory, repo)
}

struct Answer {
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

fn run(arguments: &[&str], input: Option<&Value>) -> Answer {
    let mut command = provenance();
    command.args(arguments);
    if let Some(input) = input {
        command.write_stdin(serde_json::to_vec(input).unwrap());
    }
    let output = command.output().unwrap();
    Answer {
        code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).trim().to_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
    }
}

fn route(repo: &str, route: &str, input: &Value) -> Answer {
    run(
        &[
            "api", route, "--method", "post", "--input", "-", "--repo", repo, "--scope",
            "default", "--format", "json",
        ],
        Some(input),
    )
}

fn api(repo: &str, path: &str, method: &str, body: Option<&Value>) -> Value {
    let mut arguments = vec!["api", path, "--repo", repo, "--format", "json"];
    if body.is_some() {
        arguments.extend(["--method", method, "--input", "-"]);
    }
    let answer = run(&arguments, body);
    assert_eq!(answer.code, Some(0), "{path}: {}", answer.stderr);
    serde_json::from_str(&answer.stdout).unwrap()
}

fn requirement_id(applied: &Value, key: &str) -> String {
    applied["data"]["resources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|resource| resource["kind"] == "requirement" && resource["key"] == key)
        .unwrap_or_else(|| panic!("no requirement {key} in {applied}"))["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// Applies the first document and gives the id of `sharing`.
fn typed_requirement(repo: &str) -> String {
    let applied = json_stdin_output(
        &[
            "api", "authoring-changes", "--method", "post", "--input", "-", "--repo", repo,
            "--scope", "default", "--format", "json",
        ],
        &document(&both("The host stops.")),
    );
    requirement_id(&applied, "sharing")
}

fn create(repo: &str, id: &str, kind: &str, fields: &[&str]) {
    let mut arguments = vec![id, "create", "--type", kind, "--repo", repo, "--format", "json"];
    arguments.extend_from_slice(fields);
    json_output(&arguments);
}

fn submit(repo: &str, id: &str, declared_by: Option<&str>) -> String {
    let body = json!({
        "actor": "agent", "declared_by": declared_by, "title": "Review",
        "summary": "Review the record.", "confidence": null,
        "source_ids": [], "evidence_references": [], "builds_on": [],
        "expected_revision": null, "revises": null
    });
    let answer = run(
        &[id, "submit", "--repo", repo, "--format", "json", "--stdin"],
        Some(&body),
    );
    assert_eq!(answer.code, Some(0), "submit {id}: {}", answer.stderr);
    let submitted: Value = serde_json::from_str(&answer.stdout).unwrap();
    submitted["data"]["proposal_id"].as_str().unwrap().to_owned()
}

fn pending(repo: &str, collection: &str, id: &str) -> String {
    api(repo, &format!("{collection}/{id}"), "get", None)["data"]["decision"]["pending"]
        ["proposal_id"]
        .as_str()
        .unwrap_or_else(|| panic!("{id} has no pending submission"))
        .to_owned()
}

fn decide(
    repo: &str,
    collection: &str,
    kind: &str,
    id: &str,
    proposal: &str,
    accept: bool,
    declared_by: Option<&str>,
) {
    let body = if accept {
        json!({
            "actor": {"identity_type": "human", "id": "reviewer"},
            "decision": "accepted",
            "canonical_artifact": {"artifact_type": kind, "artifact_id": id}
        })
    } else {
        json!({
            "actor": {"identity_type": "human", "id": "reviewer"},
            "decision": "rejected",
            "rationale": "The statement needs one precise condition.",
            "canonical_artifact": null,
            "feedback": {"role": "user", "body": "Name the condition."},
            "declared_by": declared_by
        })
    };
    let path = format!("{collection}/{id}/submissions/{proposal}/decide");
    api(repo, &path, "post", Some(&body));
}

fn withdraw(repo: &str, collection: &str, id: &str, proposal: &str, declared_by: Option<&str>) {
    let path = format!("{collection}/{id}/submissions/{proposal}/withdraw");
    api(repo, &path, "post", Some(&json!({"actor": "agent", "declared_by": declared_by})));
}

/// Runs the plan, then the apply, of the deletion document and prints both
/// answers and the review state of each named record before them.
fn observe(case: &str, repo: &str, records: &[(&str, &str)]) -> (Answer, Answer) {
    for (collection, id) in records {
        let state = api(repo, &format!("{collection}/{id}"), "get", None);
        eprintln!(
            "=== 0STR {case} STATE {collection}/{id}: status={} decision={}",
            state["data"]["status"], state["data"]["decision"]
        );
    }
    let plan = route(repo, "authoring-plans", &after());
    let apply = route(repo, "authoring-changes", &after());
    for (name, answer) in [("PLAN", &plan), ("APPLY", &apply)] {
        eprintln!(
            "=== 0STR {case} {name} exit={:?}\nstdout: {}\nstderr: {}",
            answer.code, answer.stdout, answer.stderr
        );
    }
    (plan, apply)
}

fn deleted_count(answer: &Answer) -> Value {
    serde_json::from_str::<Value>(&answer.stdout).map_or(Value::Null, |v| v["data"]["deleted"].clone())
}

fn shared_resolution(repo: &str, id: &str, requirement: &str) {
    let ids = json!([requirement]).to_string();
    create(
        repo,
        id,
        "resolution",
        &[
            "--title", "Shared decision", "--position", "The host stops.", "--rationale",
            "The host stops.", "--requirement-ids-json", &ids,
        ],
    );
}

fn shared_rule(repo: &str, id: &str, requirement: &str) {
    let ids = json!([requirement]).to_string();
    create(
        repo,
        id,
        "rule",
        &["--statement", "The host stops.", "--requirement-ids-json", &ids],
    );
}

fn shared_topic(repo: &str, id: &str, requirement: &str) {
    create(
        repo,
        id,
        "topic",
        &["--title", "Removed topic", "--requirement-id", requirement],
    );
}

fn shared_question(repo: &str, id: &str, topic: &str) {
    create(
        repo,
        id,
        "question",
        &[
            "--topic-id", topic, "--question", "Removed?", "--resolution-method", "research",
        ],
    );
}

#[test]
fn report_records_made_with_create_below_the_deleted_requirement() {
    let (_directory, repo) = initialized_repo();
    let removed = typed_requirement(&repo);
    shared_resolution(&repo, "res_only", &removed);
    shared_topic(&repo, "topic_only", &removed);
    shared_question(&repo, "question_only", "topic_only");
    let (plan, apply) = observe(
        "report",
        &repo,
        &[
            ("resolutions", "res_only"),
            ("topics", "topic_only"),
            ("questions", "question_only"),
        ],
    );
    assert_eq!(plan.code, Some(0));
    assert_eq!(deleted_count(&plan), json!(4));
    assert_ne!(apply.code, Some(0));
}

#[test]
fn rule_made_with_create_below_the_deleted_requirement() {
    let (_directory, repo) = initialized_repo();
    let removed = typed_requirement(&repo);
    shared_rule(&repo, "rule_only", &removed);
    observe("create_rule", &repo, &[("rules", "rule_only")]);
}

#[test]
fn resolution_made_with_create_below_the_deleted_requirement() {
    let (_directory, repo) = initialized_repo();
    let removed = typed_requirement(&repo);
    shared_resolution(&repo, "res_only", &removed);
    observe("create_resolution", &repo, &[("resolutions", "res_only")]);
}

#[test]
fn topic_made_with_create_below_the_deleted_requirement() {
    let (_directory, repo) = initialized_repo();
    let removed = typed_requirement(&repo);
    shared_topic(&repo, "topic_only", &removed);
    observe("create_topic", &repo, &[("topics", "topic_only")]);
}

#[test]
fn accepted_resolution_made_with_create_below_the_deleted_requirement() {
    let (_directory, repo) = initialized_repo();
    let removed = typed_requirement(&repo);
    shared_resolution(&repo, "res_only", &removed);
    let proposal = pending(&repo, "resolutions", "res_only");
    decide(&repo, "resolutions", "resolution", "res_only", &proposal, true, None);
    observe("create_resolution_accepted", &repo, &[("resolutions", "res_only")]);
}

#[test]
fn rejected_resolution_made_with_create_below_the_deleted_requirement() {
    let (_directory, repo) = initialized_repo();
    let removed = typed_requirement(&repo);
    shared_resolution(&repo, "res_only", &removed);
    let proposal = pending(&repo, "resolutions", "res_only");
    decide(&repo, "resolutions", "resolution", "res_only", &proposal, false, None);
    observe("create_resolution_rejected", &repo, &[("resolutions", "res_only")]);
}

#[test]
fn withdrawn_resolution_made_with_create_below_the_deleted_requirement() {
    let (_directory, repo) = initialized_repo();
    let removed = typed_requirement(&repo);
    shared_resolution(&repo, "res_only", &removed);
    let proposal = pending(&repo, "resolutions", "res_only");
    withdraw(&repo, "resolutions", "res_only", &proposal, None);
    observe("create_resolution_withdrawn", &repo, &[("resolutions", "res_only")]);
}

#[test]
fn typed_requirement_without_submission() {
    let (_directory, repo) = initialized_repo();
    let removed = typed_requirement(&repo);
    let (plan, apply) = observe("typed_control", &repo, &[("requirements", &removed)]);
    assert_eq!((plan.code, apply.code), (Some(0), Some(0)));
}

#[test]
fn typed_requirement_with_pending_submission() {
    let (_directory, repo) = initialized_repo();
    let removed = typed_requirement(&repo);
    submit(&repo, &removed, Some(OWNER));
    observe("typed_submitted", &repo, &[("requirements", &removed)]);
}

#[test]
fn typed_requirement_with_accepted_submission() {
    let (_directory, repo) = initialized_repo();
    let removed = typed_requirement(&repo);
    let proposal = submit(&repo, &removed, Some(OWNER));
    decide(&repo, "requirements", "requirement", &removed, &proposal, true, Some(OWNER));
    observe("typed_accepted", &repo, &[("requirements", &removed)]);
}

#[test]
fn typed_requirement_with_rejected_submission() {
    let (_directory, repo) = initialized_repo();
    let removed = typed_requirement(&repo);
    let proposal = submit(&repo, &removed, Some(OWNER));
    decide(&repo, "requirements", "requirement", &removed, &proposal, false, Some(OWNER));
    observe("typed_rejected", &repo, &[("requirements", &removed)]);
}

#[test]
fn typed_requirement_with_withdrawn_submission() {
    let (_directory, repo) = initialized_repo();
    let removed = typed_requirement(&repo);
    let proposal = submit(&repo, &removed, Some(OWNER));
    withdraw(&repo, "requirements", &removed, &proposal, Some(OWNER));
    observe("typed_withdrawn", &repo, &[("requirements", &removed)]);
}

#[test]
fn typed_requirement_changed_by_a_second_typed_document() {
    let (_directory, repo) = initialized_repo();
    let removed = typed_requirement(&repo);
    let changed = route(
        &repo,
        "authoring-changes",
        &document(&both("The system keeps each record.")),
    );
    eprintln!(
        "=== 0STR typed_changed CHANGE exit={:?}\nstdout: {}\nstderr: {}",
        changed.code, changed.stdout, changed.stderr
    );
    observe("typed_changed", &repo, &[("requirements", &removed)]);
}
