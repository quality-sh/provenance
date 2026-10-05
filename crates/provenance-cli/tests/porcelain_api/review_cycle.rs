use super::*;

/// The reviewer that `provenance init` takes from the fixture's Git identity.
pub(super) const REVIEWER: &str = "reviewer_example_com";

/// A repository whose Git identity gives `provenance init` its reviewer.
pub(super) fn init_with_reviewer() -> (tempfile::TempDir, String) {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_string_lossy().into_owned();
    super::review_history::git(&repo, &["init", "-q"]);
    super::review_history::git(&repo, &["config", "user.email", "reviewer@example.com"]);
    success(&[
        "init",
        "--path",
        &repo,
        "--scope",
        "default",
        "--path-prefix",
        ".",
    ]);
    (directory, repo)
}

/// Sends one write through the public API and returns its envelope.
pub(super) fn write(
    repo: &str,
    method: &str,
    path: &str,
    body: &Value,
    etag: Option<&str>,
) -> Value {
    let mut command = provenance();
    command.args([
        "api", path, "--repo", repo, "--method", method, "--input", "-",
    ]);
    if let Some(etag) = etag {
        command.args(["--header", &format!("If-Match: {etag}")]);
    }
    let result = command.write_stdin(body.to_string()).output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    serde_json::from_slice(&result.stdout).unwrap()
}

/// Changes the description of one Requirement with its current etag.
pub(super) fn describe(repo: &str, id: &str, description: &str) -> Value {
    let path = format!("requirements/{id}");
    let read = json(&["api", &path, "--repo", repo]);
    let etag = read["data"]["edit"]["etag"].as_str().unwrap().to_owned();
    write(
        repo,
        "patch",
        &path,
        &json!({"actor":"agent", "description":description}),
        Some(&etag),
    )
}

/// The decision body of the configured reviewer.
pub(super) fn decision(decision: &str, id: &str, feedback: Option<&str>) -> Value {
    let accepted = decision == "accepted";
    json!({
        "actor":{"identity_type":"human","id":REVIEWER}, "decision":decision,
        "rationale":(!accepted).then_some("Name the scope."),
        "canonical_artifact":accepted.then(|| json!({"artifact_type":"requirement","artifact_id":id})),
        "feedback":feedback.map(|body| json!({"role":"user","body":body})),
        "declared_by":null
    })
}

fn pending(written: &Value) -> String {
    written["data"]["decision"]["pending"]["proposal_id"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// Flow: create, reject with feedback, revise, withdraw, resubmit and accept;
/// the record read then shows the decisions in cycle order, the feedback and
/// the withdrawal.
#[test]
#[verifies("rule_revised_item_requires_new_review", examples)]
fn review_cycle_reads_decisions_feedback_and_withdrawal() {
    let (_directory, repo) = init_with_reviewer();
    let created = write(
        &repo,
        "post",
        "requirements",
        &json!({"actor":"agent", "id":"req_cycle", "statement":"The cycle statement applies.",
            "status":"active", "depends_on":[], "supersedes":[]}),
        None,
    );
    let first = pending(&created);
    let rejected = write(
        &repo,
        "post",
        &format!("requirements/req_cycle/submissions/{first}/decide"),
        &decision("rejected", "req_cycle", Some("Name the scope.")),
        None,
    );
    let revised = describe(&repo, "req_cycle", "The scope is the cycle.");
    let second = pending(&revised);
    write(
        &repo,
        "post",
        &format!("requirements/req_cycle/submissions/{second}/withdraw"),
        &json!({"actor":"agent", "declared_by":null, "reason":"Resubmit as an answer."}),
        None,
    );
    let resubmitted = write(
        &repo,
        "post",
        "requirements/req_cycle/submit",
        &json!({"actor":"agent", "declared_by":null, "title":"Review again",
            "summary":"Review the revised record.", "confidence":null, "source_ids":[],
            "evidence_references":[], "builds_on":[],
            "expected_revision":revised["data"]["edit"]["revision"], "revises":first}),
        None,
    );
    let third = resubmitted["data"]["proposal_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let accepted = write(
        &repo,
        "post",
        &format!("requirements/req_cycle/submissions/{third}/decide"),
        &decision("accepted", "req_cycle", None),
        None,
    );

    let state =
        json(&["api", "requirements/req_cycle", "--repo", &repo])["data"]["decision"].clone();
    let decided = state["decisions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|recorded| recorded["disposition"]["id"].clone())
        .collect::<Vec<_>>();
    assert_eq!(
        decided,
        [
            rejected["data"]["disposition_id"].clone(),
            accepted["data"]["disposition_id"].clone()
        ]
    );
    assert!(rejected["data"]["feedback_message_id"].is_string());
    assert_eq!(
        state["decisions"][0]["feedback_message_id"],
        rejected["data"]["feedback_message_id"]
    );
    assert_eq!(state["withdrawn"], json!([second]));
    assert_eq!(
        state["current_acceptance"]["disposition"]["proposal_id"],
        third
    );
}
