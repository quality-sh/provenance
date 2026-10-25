//! Every review record update takes its edit guard as `--if-match`. A stale
//! etag gets the edit conflict of the kind and a current one writes.
//! Requirement, Topic and Question updates require the guard; the other kinds
//! write without it.

use predicates::prelude::*;
use provenance_macros::verifies;
use serde_json::{json, Value};

mod support {
    pub mod review_paths;
}
use support::review_paths::{init, json_output, provenance};

/// One record of every review kind.
fn records() -> (tempfile::TempDir, String) {
    let (directory, repo) = init();
    for (collection, body) in [
        (
            "sources",
            json!({"id":"source_guard", "name":"Source", "source_type":"policy"}),
        ),
        (
            "requirements",
            json!({"id":"req_guard", "statement":"The system stores records."}),
        ),
        (
            "resolutions",
            json!({"id":"resolution_guard", "title":"Decision", "position":"Store records.",
                "rationale":"Records are necessary.", "requirement_ids":["req_guard"]}),
        ),
        (
            "rules",
            json!({"id":"rule_guard", "statement":"The system stores records.",
                "requirement_ids":["req_guard"]}),
        ),
        ("domains", json!({"id":"domain_guard", "name":"Guard"})),
        (
            "boundaries",
            json!({"id":"boundary_guard", "requirement_id":"req_guard",
                "statement":"Keep the work in the repository."}),
        ),
        (
            "topics",
            json!({"id":"topic_guard", "requirement_id":"req_guard", "title":"Guard"}),
        ),
        (
            "questions",
            json!({"id":"question_guard", "topic_id":"topic_guard",
                "question":"Is the work complete?", "resolution_method":"research"}),
        ),
    ] {
        provenance()
            .args([collection, "create", "--repo", &repo, "--stdin"])
            .write_stdin(body.to_string())
            .assert()
            .success();
    }
    (directory, repo)
}

fn etag(repo: &str, collection: &str, id: &str) -> String {
    let read =
        json_output(provenance().args([collection, id, "get", "--repo", repo, "--format", "json"]));
    read["data"]["edit"]["etag"].as_str().unwrap().to_owned()
}

fn update(repo: &str, collection: &str, id: &str, guard: Option<&str>, change: &[&str]) -> Value {
    let mut command = provenance();
    command.args([collection, id, "update", "--repo", repo, "--format", "json"]);
    if let Some(etag) = guard {
        command.args(["--if-match", etag]);
    }
    let output = command.args(change).output().unwrap();
    let stream = if output.status.success() {
        output.stdout
    } else {
        output.stderr
    };
    let mut value: Value = serde_json::from_slice(&stream).unwrap();
    value["succeeded"] = json!(output.status.success());
    value
}

/// A current `--if-match` writes, the same etag again gets the edit conflict
/// of the kind, and the record keeps the first change.
fn assert_guarded(collection: &str, id: &str, flag: &str, conflict: &str) {
    let (_directory, repo) = records();
    let current = etag(&repo, collection, id);

    let written = update(
        &repo,
        collection,
        id,
        Some(&current),
        &[flag, "The first change holds."],
    );
    assert_eq!(written["succeeded"], true, "{collection}: {written}");
    let stale = update(
        &repo,
        collection,
        id,
        Some(&current),
        &[flag, "The stale change holds."],
    );
    assert_eq!(stale["succeeded"], false, "{collection}: {stale}");
    assert_eq!(stale["error"]["kind"], conflict, "{collection}: {stale}");
    let after = json_output(
        provenance().args([collection, id, "get", "--repo", &repo, "--format", "json"]),
    );
    let field = flag.trim_start_matches("--");
    assert_eq!(
        after["data"][field], "The first change holds.",
        "{collection}: {after}"
    );
}

#[test]
#[verifies("rule_review_update_guard_header", examples)]
fn a_rule_update_takes_the_guard_as_if_match() {
    assert_guarded(
        "rules",
        "rule_guard",
        "--description",
        "record_edit_conflict",
    );
}

#[test]
#[verifies("rule_review_update_guard_header", examples)]
fn a_source_update_takes_the_guard_as_if_match() {
    assert_guarded("sources", "source_guard", "--name", "record_edit_conflict");
}

#[test]
#[verifies("rule_review_update_guard_header", examples)]
fn a_resolution_update_takes_the_guard_as_if_match() {
    assert_guarded(
        "resolutions",
        "resolution_guard",
        "--title",
        "record_edit_conflict",
    );
}

#[test]
#[verifies("rule_review_update_guard_header", examples)]
fn a_domain_update_takes_the_guard_as_if_match() {
    assert_guarded("domains", "domain_guard", "--name", "record_edit_conflict");
}

#[test]
#[verifies("rule_review_update_guard_header", examples)]
fn a_boundary_update_takes_the_guard_as_if_match() {
    assert_guarded(
        "boundaries",
        "boundary_guard",
        "--statement",
        "record_edit_conflict",
    );
}

#[test]
#[verifies("rule_review_update_guard_header", examples)]
fn a_requirement_update_takes_the_guard_as_if_match() {
    assert_guarded(
        "requirements",
        "req_guard",
        "--description",
        "requirement_edit_conflict",
    );
}

#[test]
#[verifies("rule_review_update_guard_header", examples)]
fn a_topic_update_takes_the_guard_as_if_match() {
    assert_guarded("topics", "topic_guard", "--title", "record_edit_conflict");
}

#[test]
#[verifies("rule_review_update_guard_header", examples)]
fn a_question_update_takes_the_guard_as_if_match() {
    assert_guarded(
        "questions",
        "question_guard",
        "--question",
        "record_edit_conflict",
    );
}

#[test]
#[verifies("rule_missing_if_match_is_required", examples)]
fn only_requirement_topic_and_question_updates_require_the_guard() {
    let (_directory, repo) = records();
    for (collection, id, flag, required) in [
        ("requirements", "req_guard", "--description", true),
        ("topics", "topic_guard", "--title", true),
        ("questions", "question_guard", "--question", true),
        ("rules", "rule_guard", "--description", false),
        ("sources", "source_guard", "--name", false),
        ("resolutions", "resolution_guard", "--title", false),
        ("domains", "domain_guard", "--name", false),
        ("boundaries", "boundary_guard", "--statement", false),
    ] {
        let result = update(
            &repo,
            collection,
            id,
            None,
            &[flag, "The unguarded change holds."],
        );
        if required {
            assert_eq!(result["succeeded"], false, "{collection}: {result}");
            assert_eq!(
                result["error"]["kind"], "invalid_input",
                "{collection}: {result}"
            );
            assert_eq!(
                result["error"]["field"], "If-Match",
                "{collection}: {result}"
            );
            assert_eq!(
                result["error"]["reason"], "required",
                "{collection}: {result}"
            );
        } else {
            assert_eq!(result["succeeded"], true, "{collection}: {result}");
        }
    }
}

// Implementation aid: no review kind keeps the body guard as a second flag.
#[test]
fn no_review_update_accepts_expected_etag() {
    let (_directory, repo) = records();
    for (collection, id) in [("rules", "rule_guard"), ("sources", "source_guard")] {
        let etag = etag(&repo, collection, id);
        provenance()
            .args([
                collection,
                id,
                "update",
                "--repo",
                &repo,
                "--expected-etag",
                &etag,
            ])
            .assert()
            .code(2)
            .stderr(predicate::str::contains("--expected-etag"));
    }
}
