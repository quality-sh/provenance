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
            id,
            "create",
            "--type",
            "rule",
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
            id,
            "create",
            "--type",
            "resolution",
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
    let path = format!("{collection}/{id}/submissions/{proposal}/decide");
    provenance()
        .args([
            "api", &path, "--method", "post", "--input", "-", "--repo", repo,
        ])
        .write_stdin(decision.to_string())
        .assert()
        .success();
}

/// Reads one record with its edit and decision state.
pub fn read(repo: &str, collection: &str, id: &str) -> Value {
    data(repo, &["api", &format!("{collection}/{id}")])
}

/// Applies a guarded update with the current edit etag.
pub fn guarded_update(repo: &str, collection: &str, id: &str, change: &[&str]) {
    let before = read(repo, collection, id);
    let etag = before["edit"]["etag"].as_str().unwrap().to_owned();
    let mut args = vec![id, "update", "--if-match", &etag];
    args.extend(change);
    data(repo, &args);
}

/// The pending submission of the current revision, opened first when the
/// record has none.
pub fn current_submission(repo: &str, collection: &str, id: &str) -> String {
    read(repo, collection, id)["decision"]["pending"]["proposal_id"]
        .as_str()
        .map_or_else(|| submit(repo, id), str::to_owned)
}

/// Accepts the current revision of a record.
pub fn accept_current(repo: &str, collection: &str, kind: &str, id: &str) -> String {
    let proposal = current_submission(repo, collection, id);
    decide(repo, collection, kind, id, &proposal, true);
    proposal
}

/// The number of review submissions stored for one record, read through
/// the Proposal list.
pub fn submissions(repo: &str, kind: &str, id: &str) -> usize {
    let key = format!("review:{kind}:{id}:");
    let listed = data(repo, &["api", "proposals", "--query", "limit=100"]);
    listed["items"]
        .as_array()
        .unwrap_or_else(|| panic!("no Proposal list: {listed}"))
        .iter()
        .filter(|proposal| proposal.to_string().contains(&key))
        .count()
}

/// Withdraws one submission from review.
pub fn withdraw(repo: &str, collection: &str, id: &str, proposal: &str) {
    let path = format!("{collection}/{id}/submissions/{proposal}/withdraw");
    provenance()
        .args([
            "api", &path, "--method", "post", "--input", "-", "--repo", repo,
        ])
        .write_stdin(json!({"actor":"agent","declared_by":null,"reason":null}).to_string())
        .assert()
        .success();
}

/// The pending status request of a record: its submission id and the
/// requested status.
pub fn pending_status_request(repo: &str, collection: &str, id: &str) -> (String, Value) {
    let record = read(repo, collection, id);
    let proposal = record["decision"]["pending"]["proposal_id"]
        .as_str()
        .unwrap_or_else(|| panic!("no pending submission: {record}"))
        .to_owned();
    let card = data(repo, &["api", &format!("proposals/{proposal}")]);
    let request = card["record_revision"]["status_request"].clone();
    assert!(!request.is_null(), "no status request: {card}");
    let status = request["status"].as_str().unwrap();
    assert_eq!(card["title"], format!("Set status to {status}"), "{card}");
    (proposal, request)
}

/// Requests a status with a guarded update, then accepts the request.
pub fn set_status(repo: &str, collection: &str, kind: &str, id: &str, change: &[&str]) {
    guarded_update(repo, collection, id, change);
    accept_status_request(repo, collection, kind, id);
}

/// Accepts the current revision of a Rule, then activates it through an
/// accepted status request.
pub fn activate_rule(repo: &str, id: &str) {
    accept_current(repo, "rules", "rule", id);
    set_status(repo, "rules", "rule", id, &["--status", "active"]);
}

/// Accepts the pending status request of a record as the reviewer.
pub fn accept_status_request(repo: &str, collection: &str, kind: &str, id: &str) {
    let (proposal, _) = pending_status_request(repo, collection, id);
    decide(repo, collection, kind, id, &proposal, true);
}

/// Accepts the current revision of a Rule, then archives it in `commit`
/// through an accepted status request.
pub fn archive_rule(repo: &str, id: &str, commit: &str) {
    accept_current(repo, "rules", "rule", id);
    set_status(
        repo,
        "rules",
        "rule",
        id,
        &["--status", "archived", "--archived-in-commit", commit],
    );
}
