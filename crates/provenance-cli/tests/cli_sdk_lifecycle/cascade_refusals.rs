//! The correction of a refused cascade deletion names each deleted
//! Requirement that keeps the record, and a refused deletion affects no Rule.

use super::plan_apply::{
    apply, create, document, resource, route, run, submit, succeeded, KEPT, OWNER,
};
use super::porcelain_authoring_support::initialized_repo;
use provenance_macros::verifies;
use serde_json::{json, Value};

const FIRST: (&str, &str) = ("first", "The host stops.");
const SECOND: (&str, &str) = ("second", "The pump stops.");

fn plan_of_kept_only(repo: &str) -> Value {
    succeeded(&route(repo, "authoring-plans", &document(&[KEPT]), true))
}

fn refused(kind: &str, id: &str, correction: &str) -> Value {
    json!({
        "kind": kind, "id": id, "state": "refused",
        "refusal": {
            "kind": "review_submission",
            "message": format!(
                "cannot delete {kind} {id} because it has a review submission; {correction}"
            )
        }
    })
}

/// A Resolution stays when one Requirement that it cites stays, so the
/// correction names the two deleted Requirements with "or".
#[test]
#[verifies("rule_typed_plan_shows_refused_deletions", examples)]
fn a_resolution_that_cites_two_deleted_requirements_names_them_with_or() {
    let (_directory, repo) = initialized_repo(None);
    let ids = apply(&repo, &[FIRST, SECOND, KEPT]);
    let (first, second) = (ids["first"].as_str(), ids["second"].as_str());
    let cites_both = json!([first, second]).to_string();
    create(
        &repo,
        "res_both",
        "resolution",
        &[
            "--title",
            "Shared decision",
            "--position",
            "The host stops.",
            "--rationale",
            "The host stops.",
            "--requirement-ids-json",
            &cites_both,
        ],
    );

    let plan = plan_of_kept_only(&repo);

    let correction =
        format!("keep requirement {first} or requirement {second} in the typed document");
    assert_eq!(
        plan["data"]["cascade"],
        json!([refused("resolution", "res_both", &correction)]),
        "{plan}"
    );
}

/// An import can give a Question a Requirement that is not the Requirement
/// of its Topic. The Question stays only when both stay, so the correction
/// names the two deleted Requirements with "and".
#[test]
#[verifies("rule_typed_plan_shows_refused_deletions", examples)]
fn a_question_that_two_deleted_requirements_keep_names_them_with_and() {
    let (_directory, repo) = initialized_repo(None);
    let ids = apply(&repo, &[FIRST, SECOND, KEPT]);
    let (first, second) = (ids["first"].as_str(), ids["second"].as_str());
    let files = tempfile::tempdir().unwrap();
    let export = files.path().join("export.json").to_string_lossy().into_owned();
    let scope = ["--repo", &repo, "--scope", "default", "--format", "json"];
    let exported = run(&[&["export"], &scope[..], &["--output", &export]].concat(), None);
    assert_eq!(exported.code, Some(0), "{}", exported.stderr);
    let mut exported: Value = serde_json::from_slice(&std::fs::read(&export).unwrap()).unwrap();
    exported["topics"] = json!([{
        "scope_id": "default", "id": "topic_second", "requirement_id": second,
        "title": "Pump topic", "status": "open", "links": []
    }]);
    exported["questions"] = json!([{
        "scope_id": "default", "id": "question_first", "topic_id": "topic_second",
        "requirement_id": first, "question": "Does the host stop?",
        "resolution_method": "research", "status": "open", "links": []
    }]);
    std::fs::write(&export, serde_json::to_vec(&exported).unwrap()).unwrap();
    let imported = run(&[&["import"], &scope[..], &["--input", &export]].concat(), None);
    assert_eq!(imported.code, Some(0), "{}", imported.stderr);
    submit(&repo, "question_first", None);

    let plan = plan_of_kept_only(&repo);

    let correction =
        format!("keep requirement {first} and requirement {second} in the typed document");
    assert_eq!(
        plan["data"]["cascade"],
        json!([
            {"kind": "topic", "id": "topic_second", "state": "deleted"},
            refused("question", "question_first", &correction),
        ]),
        "{plan}"
    );
}

/// The apply does not delete a Requirement whose deletion it refuses, so the
/// plan lists no Rule of that Requirement as affected.
#[test]
#[verifies("rule_typed_plan_shows_refused_deletions", examples)]
fn a_refused_deletion_affects_no_rule() {
    let (_directory, repo) = initialized_repo(None);
    let ids = apply(&repo, &[FIRST, KEPT]);
    let first = ids["first"].as_str();
    submit(&repo, first, Some(OWNER));
    create(
        &repo,
        "rule_first",
        "rule",
        &["--statement", "The host stops.", "--requirement-id", first],
    );

    let plan = plan_of_kept_only(&repo);

    assert_eq!(resource(&plan, "first")["state"], "refused", "{plan}");
    let correction = format!("keep requirement {first} in the typed document");
    assert_eq!(
        plan["data"]["cascade"],
        json!([refused("rule", "rule_first", &correction)]),
        "{plan}"
    );
    assert_eq!(plan["data"]["affected_rules"], json!([]), "{plan}");
}
