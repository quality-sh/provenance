//! The plan of a typed authoring document shows each deletion that the apply
//! of the same document refuses, with the cause that the apply gives.

use super::porcelain_authoring_support::provenance;
use provenance_macros::verifies;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const OWNER: &str = "spec://typescript/share-links";
const KEPT: (&str, &str) = ("kept", "The system retains records.");

fn document(requirements: &[(&str, &str)]) -> Value {
    let requirements = requirements
        .iter()
        .map(|(key, statement)| json!({"key": key, "statement": statement}))
        .collect::<Vec<_>>();
    json!({
        "spec": "share-links",
        "declared_by": OWNER,
        "sources": [],
        "requirements": requirements,
        "rules": []
    })
}

/// An initialized repository whose reviewer is `reviewer`.
fn initialized_repo() -> (tempfile::TempDir, String) {
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

/// Sends `input` to an authoring route, in the JSON form or in the text form.
fn route(repo: &str, route: &str, input: &Value, json_form: bool) -> Answer {
    let mut arguments = vec![
        "api", route, "--method", "post", "--input", "-", "--repo", repo, "--scope", "default",
    ];
    if json_form {
        arguments.extend(["--format", "json"]);
    }
    run(&arguments, Some(input))
}

fn succeeded(answer: &Answer) -> Value {
    assert_eq!(answer.code, Some(0), "{}", answer.stderr);
    serde_json::from_str(&answer.stdout).unwrap()
}

fn api(repo: &str, path: &str, body: &Value) -> Value {
    let arguments = [
        "api", path, "--repo", repo, "--format", "json", "--method", "post", "--input", "-",
    ];
    succeeded(&run(&arguments, Some(body)))
}

/// Applies `requirements` and gives the id of each key.
fn apply(repo: &str, requirements: &[(&str, &str)]) -> BTreeMap<String, String> {
    let applied = succeeded(&route(
        repo,
        "authoring-changes",
        &document(requirements),
        true,
    ));
    applied["data"]["resources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|resource| {
            let key = resource["key"].as_str().unwrap().to_owned();
            (key, resource["id"].as_str().unwrap().to_owned())
        })
        .collect()
}

fn submit(repo: &str, id: &str) -> String {
    let body = json!({
        "actor": "agent", "declared_by": OWNER, "title": "Review",
        "summary": "Review the record.", "confidence": null,
        "source_ids": [], "evidence_references": [], "builds_on": [],
        "expected_revision": null, "revises": null
    });
    let arguments = [id, "submit", "--repo", repo, "--format", "json", "--stdin"];
    succeeded(&run(&arguments, Some(&body)))["data"]["proposal_id"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// Accepts or rejects the submission and gives the answer of the decision.
fn decide(repo: &str, id: &str, proposal: &str, accept: bool) -> Value {
    let body = if accept {
        json!({
            "actor": {"identity_type": "human", "id": "reviewer"},
            "decision": "accepted",
            "canonical_artifact": {"artifact_type": "requirement", "artifact_id": id}
        })
    } else {
        json!({
            "actor": {"identity_type": "human", "id": "reviewer"},
            "decision": "rejected",
            "rationale": "The statement needs one precise condition.",
            "canonical_artifact": null,
            "feedback": {"role": "user", "body": "Name the condition."},
            "declared_by": OWNER
        })
    };
    api(
        repo,
        &format!("requirements/{id}/submissions/{proposal}/decide"),
        &body,
    )
}

fn withdraw(repo: &str, id: &str, proposal: &str) {
    let path = format!("requirements/{id}/submissions/{proposal}/withdraw");
    api(repo, &path, &json!({"actor": "agent", "declared_by": OWNER}));
}

/// The bytes of each state file below the repository. The cache is a derived
/// copy of the state, so it is left out.
fn state_files(repo: &str) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(root: &Path, directory: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            let relative = path.strip_prefix(root).unwrap().to_path_buf();
            if relative.starts_with(".provenance/cache") || relative.starts_with(".git") {
                continue;
            }
            if path.is_dir() {
                walk(root, &path, files);
            } else {
                files.insert(relative, std::fs::read(&path).unwrap());
            }
        }
    }
    let mut files = BTreeMap::new();
    walk(Path::new(repo), Path::new(repo), &mut files);
    files
}

fn resource<'a>(plan: &'a Value, key: &str) -> &'a Value {
    plan["data"]["resources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|resource| resource["key"] == key)
        .unwrap_or_else(|| panic!("no resource {key} in {plan}"))
}

fn submission_refusal(id: &str) -> Value {
    json!({
        "cause": "review_submission",
        "message": format!(
            "cannot delete requirement {id} because it has a review submission; \
             keep the requirement in the typed document"
        )
    })
}

#[test]
#[verifies("rule_typed_plan_shows_refused_deletions", examples)]
fn the_plan_shows_each_deletion_that_the_apply_refuses_with_its_cause() {
    let (_directory, repo) = initialized_repo();
    let all = [
        ("pending", "The host stops."),
        ("rejected", "The host starts."),
        ("withdrawn", "The pump stops."),
        ("accepted", "The pump starts."),
        ("plain", "The system stores records."),
        KEPT,
    ];
    let ids = apply(&repo, &all);
    submit(&repo, &ids["pending"]);
    let proposal = submit(&repo, &ids["rejected"]);
    decide(&repo, &ids["rejected"], &proposal, false);
    let proposal = submit(&repo, &ids["withdrawn"]);
    withdraw(&repo, &ids["withdrawn"], &proposal);
    let proposal = submit(&repo, &ids["accepted"]);
    let accepted = decide(&repo, &ids["accepted"], &proposal, true);
    let disposition = accepted["data"]["disposition_id"].as_str().unwrap();
    let kept_only = document(&[KEPT]);
    let before = state_files(&repo);

    let plan = succeeded(&route(&repo, "authoring-plans", &kept_only, true));
    let text = succeeded(&route(&repo, "authoring-plans", &kept_only, false));
    let refused = route(&repo, "authoring-changes", &kept_only, true);

    assert_eq!(text, plan);
    for key in ["pending", "rejected", "withdrawn"] {
        let entry = resource(&plan, key);
        assert_eq!(entry["state"], "refused", "{plan}");
        assert_eq!(entry["refusal"], submission_refusal(&ids[key]), "{plan}");
    }
    let disposition_message = format!(
        "cannot delete canonical requirement {} because disposition {disposition} names it; \
         keep the requirement in the typed document",
        ids["accepted"]
    );
    let entry = resource(&plan, "accepted");
    assert_eq!(entry["state"], "refused", "{plan}");
    assert_eq!(
        entry["refusal"],
        json!({"cause": "disposition", "disposition_id": disposition,
               "message": disposition_message}),
        "{plan}"
    );
    assert_eq!(resource(&plan, "plain")["state"], "deleted", "{plan}");
    assert!(resource(&plan, "plain").get("refusal").is_none(), "{plan}");
    assert_eq!(resource(&plan, "kept")["state"], "unchanged", "{plan}");
    let counts = ["deleted", "refused", "conflicts", "unchanged"].map(|count| &plan["data"][count]);
    assert_eq!(counts, [&json!(1), &json!(4), &json!(0), &json!(1)], "{plan}");

    assert_eq!(refused.code, Some(1), "{}", refused.stdout);
    assert_eq!(refused.stdout, "");
    let error = refused.stderr.strip_prefix("Error: ").unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(error).unwrap(),
        json!({"error": {"kind": "invalid_update", "message": disposition_message}, "meta": {}})
    );
    assert_eq!(state_files(&repo), before);
}

#[test]
#[verifies("rule_typed_plan_shows_refused_deletions", examples)]
fn a_plan_with_no_refused_deletion_applies() {
    let (_directory, repo) = initialized_repo();
    let ids = apply(&repo, &[("plain", "The system stores records."), KEPT]);
    let kept_only = document(&[KEPT]);

    let plan = succeeded(&route(&repo, "authoring-plans", &kept_only, true));
    let applied = succeeded(&route(&repo, "authoring-changes", &kept_only, true));

    assert_eq!(resource(&plan, "plain")["state"], "deleted", "{plan}");
    assert_eq!(resource(&plan, "plain")["id"], ids["plain"].as_str());
    assert_eq!(plan["data"]["refused"], 0, "{plan}");
    assert_eq!(applied["data"]["deleted"], 1, "{applied}");
    assert_eq!(applied["data"]["resources"], plan["data"]["resources"]);
    let read = run(
        &[&ids["plain"], "get", "--repo", &repo, "--format", "json"],
        None,
    );
    assert_ne!(read.code, Some(0), "{}", read.stdout);
}
