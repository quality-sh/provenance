// Each test binary that includes this file uses only some of the helpers.
#![allow(dead_code)]

use super::review_paths::{json_output, provenance};
use serde_json::{json, Value};

/// Runs one JSON command against `repo` and returns its `data` object.
pub fn data(repo: &str, args: &[&str]) -> Value {
    let mut all = args.to_vec();
    all.extend(["--repo", repo, "--format", "json"]);
    json_output(provenance().args(all))["data"].clone()
}

pub fn create_rule(repo: &str, id: &str, requirement: &str) {
    data(
        repo,
        &[
            "rules",
            "create",
            "--id",
            id,
            "--statement",
            "The system keeps each record.",
            "--requirement-id",
            requirement,
        ],
    );
}

pub fn create_resolution(repo: &str, id: &str, requirement: &str) {
    data(
        repo,
        &[
            "resolutions",
            "create",
            "--id",
            id,
            "--title",
            "Store records",
            "--position",
            "The system stores records in one file.",
            "--rationale",
            "One file is simple.",
            "--requirement-id",
            requirement,
        ],
    );
}

/// Submits the current revision of a record and returns the submission id.
pub fn submit(repo: &str, id: &str) -> String {
    let output = provenance()
        .args([id, "submit", "--repo", repo, "--format", "json", "--stdin"])
        .write_stdin(
            json!({
                "actor":"agent", "declared_by":null, "title":"Review",
                "summary":"Review the record.", "confidence":null,
                "source_ids":[], "evidence_references":[], "builds_on":[],
                "expected_revision":null, "revises":null
            })
            .to_string(),
        )
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let submitted: Value = serde_json::from_slice(&output.stdout).unwrap();
    submitted["data"]["proposal_id"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// Records a reviewer decision on one submission.
pub fn decide(repo: &str, collection: &str, kind: &str, id: &str, proposal: &str, accept: bool) {
    let decision = if accept {
        json!({
            "actor":{"identity_type":"human","id":"reviewer"},
            "decision":"accepted",
            "canonical_artifact":{"artifact_type":kind,"artifact_id":id}
        })
    } else {
        json!({
            "actor":{"identity_type":"human","id":"reviewer"},
            "decision":"rejected",
            "rationale":"The statement needs one precise condition.",
            "canonical_artifact":null,
            "feedback":{"role":"user","body":"Name the condition."},
            "declared_by":null
        })
    };
    provenance()
        .args([
            collection,
            id,
            "submissions",
            proposal,
            "decide",
            "--repo",
            repo,
            "--stdin",
        ])
        .write_stdin(decision.to_string())
        .assert()
        .success();
}

/// Reads one record with its edit and decision state.
pub fn read(repo: &str, collection: &str, id: &str) -> Value {
    data(repo, &[collection, id, "get"])
}

/// Applies a guarded update with the current edit etag.
pub fn guarded_update(repo: &str, collection: &str, id: &str, change: &[&str]) {
    let before = read(repo, collection, id);
    let etag = before["edit"]["etag"].as_str().unwrap().to_owned();
    let mut args = vec![collection, id, "update", "--if-match", &etag];
    args.extend(change);
    data(repo, &args);
}

/// Submits and accepts the current revision of a record.
pub fn accept_current(repo: &str, collection: &str, kind: &str, id: &str) -> String {
    let proposal = submit(repo, id);
    decide(repo, collection, kind, id, &proposal, true);
    proposal
}
