use super::*;
use provenance_core::Manifest;
use provenance_store::layout::ProvenanceLayout;

fn allow_reviewer(repo: &str) {
    let layout = ProvenanceLayout::new(camino::Utf8Path::new(repo));
    let mut manifest: Manifest =
        serde_json::from_slice(&std::fs::read(layout.manifest_path()).unwrap()).unwrap();
    manifest.disposition_actor_ids.push("reviewer".into());
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
}

fn api(repo: &str, path: &str, body: &Value) -> std::process::Output {
    provenance()
        .args([
            "api", path, "--repo", repo, "--method", "post", "--input", "-",
        ])
        .write_stdin(body.to_string())
        .output()
        .unwrap()
}

fn create_requirement(repo: &str) {
    success(&[
        "req_review",
        "create",
        "--type",
        "requirement",
        "--repo",
        repo,
        "--statement",
        "The review statement applies.",
    ]);
}

fn edit(repo: &str, description: &str) -> (String, String) {
    let read = json(&["api", "requirements/req_review", "--repo", repo]);
    let etag = read["data"]["edit"]["etag"].as_str().unwrap();
    let request_id: String = description
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .flat_map(char::to_lowercase)
        .collect();
    let key = format!("Idempotency-Key: {request_id}");
    let result = provenance()
        .args([
            "api",
            "requirements/req_review",
            "--repo",
            repo,
            "--method",
            "patch",
            "--input",
            "-",
            "--header",
            &key,
            "--header",
            &format!("If-Match: {etag}"),
        ])
        .write_stdin(json!({"actor":"agent","description":description}).to_string())
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let saved = serde_json::from_slice::<Value>(&result.stdout).unwrap();
    (
        saved["data"]["edit"]["revision"]
            .as_str()
            .unwrap()
            .to_owned(),
        saved["data"]["decision"]["pending"]["proposal_id"]
            .as_str()
            .unwrap()
            .to_owned(),
    )
}

fn submit(repo: &str, revision: &str) -> std::process::Output {
    api(
        repo,
        "requirements/req_review/submit",
        &json!({
            "actor":"agent", "declared_by":null, "title":"Review", "summary":"Review it.",
            "confidence":null, "source_ids":[], "evidence_references":[], "builds_on":[],
            "expected_revision":revision, "revises":null
        }),
    )
}

fn conflict(submission: Option<&str>, revision: &str) -> Value {
    json!({"error":{"kind":"review_submission_conflict",
        "current_submission":submission,"current_revision":revision},"meta":{}})
}

fn successful(output: &std::process::Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn refused(output: &std::process::Output) -> Value {
    assert!(!output.status.success());
    envelope(&String::from_utf8_lossy(&output.stderr))
}

#[test]
fn cli_submit_conflicts_return_the_typed_envelope() {
    let (_directory, repo) = init();
    create_requirement(&repo);
    let (revision_1, _) = edit(&repo, "Revision one.");
    let (revision_2, automatic) = edit(&repo, "Revision two.");
    assert_eq!(
        refused(&submit(&repo, &revision_1)),
        conflict(Some(&automatic), &revision_2)
    );
    assert_eq!(
        refused(&submit(&repo, &revision_2)),
        conflict(Some(&automatic), &revision_2)
    );
    successful(&api(
        &repo,
        &format!("requirements/req_review/submissions/{automatic}/withdraw"),
        &json!({"actor":"agent","declared_by":null,"reason":null}),
    ));
    let submitted = successful(&submit(&repo, &revision_2));
    let proposal = submitted["data"]["proposal_id"].as_str().unwrap();
    assert_eq!(
        refused(&submit(&repo, &revision_2)),
        conflict(Some(proposal), &revision_2)
    );
}

fn terminal(repo: &str, proposal: &str, action: &str) -> Value {
    let body = match action {
        "decide" => json!({"actor":{"identity_type":"human","id":"reviewer"},
            "decision":"accepted", "rationale":null,
            "canonical_artifact":{"artifact_type":"requirement","artifact_id":"req_review"},
            "feedback":null, "declared_by":null}),
        _ => json!({"actor":"agent","declared_by":null,"reason":null}),
    };
    refused(&api(
        repo,
        &format!("requirements/req_review/submissions/{proposal}/{action}"),
        &body,
    ))
}

fn prepared(state: &str) -> (tempfile::TempDir, String, String, String, Option<String>) {
    let (directory, repo) = init();
    create_requirement(&repo);
    allow_reviewer(&repo);
    let (mut revision, proposal) = edit(&repo, "Revision one.");
    let mut current = None;
    if state == "stale" {
        let edited = edit(&repo, "Revision two.");
        revision = edited.0;
        current = Some(edited.1);
    } else if state == "withdrawn" {
        successful(&api(
            &repo,
            &format!("requirements/req_review/submissions/{proposal}/withdraw"),
            &json!({"actor":"agent","declared_by":null,"reason":null}),
        ));
    } else {
        successful(&api(
            &repo,
            &format!("requirements/req_review/submissions/{proposal}/decide"),
            &json!({"actor":{"identity_type":"human","id":"reviewer"}, "decision":"rejected",
                "rationale":"Needs work.", "canonical_artifact":null,
                "feedback":null, "declared_by":null}),
        ));
    }
    (directory, repo, proposal, revision, current)
}

#[test]
fn cli_terminal_review_conflicts_share_one_envelope() {
    for state in ["stale", "withdrawn", "decided"] {
        let (_directory, repo, proposal, revision, current) = prepared(state);
        for action in ["decide", "withdraw"] {
            assert_eq!(
                terminal(&repo, &proposal, action),
                conflict(current.as_deref(), &revision)
            );
        }
    }
}
