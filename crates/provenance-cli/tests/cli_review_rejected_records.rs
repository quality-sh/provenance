#[path = "review_host_support/mod.rs"]
mod review_host_support;

use provenance_macros::verifies;
use review_host_support::{repository_with_actors, request, response, start, Host};
use serde_json::{json, Value};
use std::path::Path;

fn provenance(root: &Path, args: &[&str]) {
    let output = std::process::Command::new(assert_cmd::cargo::cargo_bin("provenance"))
        .args(args)
        .arg("--repo")
        .arg(root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn create_resolution(root: &Path, id: &str) {
    provenance(
        root,
        &[
            "resolutions",
            "create",
            "--id",
            id,
            "--title",
            "Keep the result",
            "--position",
            "The system stores each result.",
            "--rationale",
            "The result must remain available.",
            "--status",
            "proposed",
            "--made-by",
            "shaping_agent",
            "--requirement-ids",
            "req_hide",
        ],
    );
}

fn repository() -> tempfile::TempDir {
    let repo = repository_with_actors(&["reviewer"]);
    provenance(
        repo.path(),
        &[
            "requirements",
            "create",
            "--id",
            "req_hide",
            "--statement",
            "The system stores each result.",
        ],
    );
    create_resolution(repo.path(), "res_new");
    create_resolution(repo.path(), "res_changed");
    repo
}

fn post(host: &Host, path: &str, data: &Value) -> Value {
    let result = response(
        request(host, "POST", path, true)
            .set("Content-Type", "application/json")
            .send_string(&json!({"data":data}).to_string()),
    );
    let status = result.status();
    let body: Value = serde_json::from_str(&result.into_string().unwrap()).unwrap();
    assert_eq!(status, 200, "{body}");
    body
}

fn get(host: &Host, path: &str) -> Value {
    let result = request(host, "GET", path, true).call().unwrap();
    serde_json::from_str::<Value>(&result.into_string().unwrap()).unwrap()["data"].clone()
}

/// Returns the waiting submission, and submits the record when none waits.
fn pending(host: &Host, id: &str) -> String {
    let waiting = &get(host, &format!("/resolutions/{id}"))["decision"]["pending"];
    if let Some(proposal) = waiting["proposal_id"].as_str() {
        return proposal.to_owned();
    }
    post(
        host,
        &format!("/resolutions/{id}/submit"),
        &json!({
            "actor":"shaping_agent", "declared_by":"shaping_agent", "title":"Keep the result",
            "summary":"The system stores each result.", "source_ids":[],
            "evidence_references":[], "builds_on":[]
        }),
    );
    get(host, &format!("/resolutions/{id}"))["decision"]["pending"]["proposal_id"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// Decides the waiting submission with the body that the review page sends.
fn decide(host: &Host, id: &str, decision: &str) {
    let proposal = pending(host, id);
    let body = if decision == "rejected" {
        json!({
            "actor":{"identity_type":"human","id":"reviewer"},
            "decision":"rejected", "rationale":"The result needs a clear condition.",
            "feedback":{"body":"The result needs a clear condition.","role":"user"}
        })
    } else {
        json!({
            "actor":{"identity_type":"human","id":"reviewer"},
            "decision":"accepted", "rationale":null,
            "canonical_artifact":{"artifact_type":"resolution","artifact_id":id},
            "feedback":null
        })
    };
    post(
        host,
        &format!("/resolutions/{id}/submissions/{proposal}/decide"),
        &body,
    );
}

/// Rejects the new Resolution, and accepts then rejects a change to the other.
fn reviewed() -> (tempfile::TempDir, Host) {
    let repo = repository();
    let host = start(repo.path());
    decide(&host, "res_new", "rejected");
    decide(&host, "res_changed", "accepted");
    provenance(
        repo.path(),
        &[
            "resolutions",
            "res_changed",
            "update",
            "--position",
            "The system stores each result for one year.",
        ],
    );
    decide(&host, "res_changed", "rejected");
    (repo, host)
}

fn document(host: &Host, exclude_terminal: bool) -> Value {
    get(
        host,
        &format!("/requirements/req_hide/document?exclude_terminal={exclude_terminal}"),
    )
}

fn entries<'a>(document: &'a Value, id: &str) -> Vec<&'a Value> {
    document["entries"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| entry["node"]["id"] == id)
        .collect()
}

fn outcomes(id: &str, host: &Host) -> Vec<String> {
    get(host, &format!("/resolutions/{id}"))["decision"]["decisions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|decision| {
            decision["disposition"]["decision"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect()
}

#[test]
#[verifies("rule_review_document_hides_rejected_new_records", examples)]
fn page_rejection_of_a_new_record_removes_it_from_the_review_document() {
    let (_repo, host) = reviewed();

    let current = document(&host, true);

    assert!(entries(&current, "res_new").is_empty(), "{current}");
    // The rejection feedback Discussion leaves with its record.
    assert!(!current.to_string().contains("res_new"), "{current}");
    assert_eq!(current["review_totals"]["rejected"], 1, "{current}");
}

#[test]
#[verifies("rule_review_document_hides_rejected_new_records", examples)]
#[verifies("rule_rejection_keeps_graph_record", examples)]
fn show_option_and_decision_history_keep_the_rejected_new_record_reachable() {
    let (_repo, host) = reviewed();

    let all = document(&host, false);

    let shown = entries(&all, "res_new");
    assert_eq!(shown.len(), 1, "{all}");
    assert_eq!(shown[0]["review"]["outcome"], "rejected");
    assert_eq!(all["review_totals"]["rejected"], 2, "{all}");
    let record = get(&host, "/resolutions/res_new");
    assert_eq!(record["status"], "proposed");
    assert_eq!(outcomes("res_new", &host), ["rejected"]);
}

#[test]
#[verifies("rule_review_document_hides_rejected_new_records", examples)]
fn rejected_change_keeps_the_record_once_with_its_current_content() {
    let (_repo, host) = reviewed();

    let current = document(&host, true);

    let shown = entries(&current, "res_changed");
    assert_eq!(shown.len(), 1, "{current}");
    let record = get(&host, "/resolutions/res_changed");
    assert_eq!(
        shown[0]["node"]["position"],
        "The system stores each result for one year."
    );
    assert_eq!(shown[0]["node"]["position"], record["position"]);
    assert_eq!(outcomes("res_changed", &host), ["accepted", "rejected"]);
}

#[test]
#[verifies("rule_withdrawal_preserves_review_history", examples)]
#[verifies("rule_rejected_item_returns_to_agent", examples)]
fn rejected_candidate_can_be_withdrawn_with_feedback_and_history_intact() {
    let repo = repository();
    let host = start(repo.path());
    let proposal = pending(&host, "res_new");
    decide(&host, "res_new", "rejected");
    let before = get(&host, "/resolutions/res_new");
    let feedback = get(&host, "/resolutions/res_new/discussions");
    assert!(feedback.to_string().contains("The result needs a clear condition."));

    post(
        &host,
        &format!("/resolutions/res_new/submissions/{proposal}/withdraw"),
        &json!({"actor":"shaping_agent", "declared_by":"shaping_agent",
            "reason":"The candidate needs a different approach."}),
    );

    let after = get(&host, "/resolutions/res_new");
    assert_eq!(after["decision"]["decisions"], before["decision"]["decisions"]);
    assert_eq!(outcomes("res_new", &host), ["rejected"]);
    assert_eq!(after["decision"]["withdrawn"], json!([proposal]));
    assert!(after["decision"]["pending"].is_null());
    assert_eq!(get(&host, "/resolutions/res_new/discussions"), feedback);
    assert!(entries(&document(&host, true), "res_new").is_empty());
}

#[test]
#[verifies("rule_withdrawal_preserves_review_history", examples)]
fn accepted_candidate_cannot_be_withdrawn() {
    let repo = repository();
    let host = start(repo.path());
    let proposal = pending(&host, "res_new");
    decide(&host, "res_new", "accepted");
    let before = get(&host, "/resolutions/res_new");

    let result = response(
        request(&host, "POST", &format!("/resolutions/res_new/submissions/{proposal}/withdraw"), true)
            .set("Content-Type", "application/json")
            .send_string(&json!({"data":{"actor":"shaping_agent",
                "declared_by":"shaping_agent", "reason":"Withdraw the candidate."}}).to_string()),
    );

    assert_eq!(result.status(), 409);
    assert_eq!(get(&host, "/resolutions/res_new"), before);
}
