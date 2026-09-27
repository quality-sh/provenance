use super::*;
use provenance_core::Manifest;

fn allow_reviewer(repository: &Repository) {
    let mut manifest: Manifest = serde_json::from_slice(
        &std::fs::read(repository.layout.manifest_path()).unwrap(),
    ).unwrap();
    manifest.disposition_actor_ids.push("reviewer".into());
    std::fs::write(
        repository.layout.manifest_path(), serde_json::to_vec(&manifest).unwrap(),
    ).unwrap();
}

async fn edit(session: &ApiSession, description: &str) -> String {
    let read = session.call(json!({"path":"requirements/req_shared"})).await;
    let etag = read.structured_content.as_ref().unwrap()["data"]["edit"]["etag"]
        .as_str().unwrap();
    let saved = session.call(json!({
        "path":"requirements/req_shared", "method":"patch",
        "headers":{"Idempotency-Key":format!("edit-{description}"),"If-Match":etag},
        "body":{"actor":"agent","description":description}
    })).await;
    assert_ne!(saved.is_error, Some(true), "{saved:?}");
    saved.structured_content.as_ref().unwrap()["data"]["edit"]["revision"]
        .as_str().unwrap().to_owned()
}

async fn submit(session: &ApiSession, revision: &str) -> CallToolResult {
    session.call(json!({
        "path":"requirements/req_shared/submit", "method":"post",
        "body":{"actor":"agent", "declared_by":null, "title":"Review", "summary":"Review it.",
            "confidence":null, "source_ids":[], "evidence_references":[], "builds_on":[],
            "expected_revision":revision, "revises":null}
    })).await
}

fn conflict(submission: Option<&str>, revision: &str) -> Value {
    json!({"error":{"kind":"review_submission_conflict",
        "current_submission":submission,"current_revision":revision},"meta":{}})
}

#[tokio::test]
async fn mcp_submit_conflicts_return_the_typed_envelope() {
    let repository = Repository::new("The shared graph is readable.");
    let session = ApiSession::start(StatementHost::with_fixture_access(
        access(&repository).allow_writes(),
    )).await;
    let revision_1 = edit(&session, "revision-one").await;
    let revision_2 = edit(&session, "revision-two").await;
    let stale = submit(&session, &revision_1).await;
    assert_eq!(stale.is_error, Some(true));
    assert_eq!(stale.structured_content.unwrap(), conflict(None, &revision_2));

    let submitted = submit(&session, &revision_2).await;
    let proposal = submitted.structured_content.as_ref().unwrap()["data"]["proposal_id"]
        .as_str().unwrap().to_owned();
    let repeated = submit(&session, &revision_2).await;
    assert_eq!(repeated.is_error, Some(true));
    assert_eq!(repeated.structured_content.unwrap(), conflict(Some(&proposal), &revision_2));
    let proposals = session.call(json!({"path":"proposals"})).await;
    assert_eq!(proposals.structured_content.unwrap()["data"]["items"].as_array().unwrap().len(), 1);
    session.shutdown().await;
}

async fn terminal(session: &ApiSession, proposal: &str, action: &str) -> CallToolResult {
    let body = match action {
        "decide" => json!({"actor":{"identity_type":"human","id":"reviewer"},
            "decision":"accepted", "rationale":null,
            "canonical_artifact":{"artifact_type":"requirement","artifact_id":"req_shared"},
            "feedback":null, "declared_by":null}),
        _ => json!({"actor":"agent","declared_by":null,"reason":null}),
    };
    session.call(json!({"path":format!(
        "requirements/req_shared/submissions/{proposal}/{action}"),
        "method":"post", "body":body})).await
}

async fn prepared(state: &str) -> (Repository, ApiSession, String, String) {
    let repository = Repository::new("The shared graph is readable.");
    allow_reviewer(&repository);
    let session = ApiSession::start(StatementHost::with_fixture_access(
        access(&repository).allow_writes(),
    )).await;
    let mut revision = edit(&session, "revision-one").await;
    let submitted = submit(&session, &revision).await;
    let proposal = submitted.structured_content.as_ref().unwrap()["data"]["proposal_id"]
        .as_str().unwrap().to_owned();
    if state == "stale" {
        revision = edit(&session, "revision-two").await;
    } else {
        let action = if state == "withdrawn" { "withdraw" } else { "decide" };
        let terminal = terminal(&session, &proposal, action).await;
        assert_ne!(terminal.is_error, Some(true), "{terminal:?}");
    }
    (repository, session, proposal, revision)
}

#[tokio::test]
async fn mcp_terminal_review_conflicts_share_one_envelope() {
    for state in ["stale", "withdrawn", "decided"] {
        let (_repository, session, proposal, revision) = prepared(state).await;
        for action in ["decide", "withdraw"] {
            let refused = terminal(&session, &proposal, action).await;
            assert_eq!(refused.is_error, Some(true), "{refused:?}");
            assert_eq!(refused.structured_content.unwrap(), conflict(None, &revision));
        }
        session.shutdown().await;
    }
}
