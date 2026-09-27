use super::*;
use provenance_core::Manifest;

async fn enroll(host: &StatementHost) -> String {
    let (status, read, etag) = call(host, "GET", "/requirements/req_shared", None, &[]).await;
    assert_eq!(status, 200, "{read}");
    let etag = etag.unwrap();
    let (status, saved, _) = call(
        host,
        "PATCH",
        "/requirements/req_shared",
        Some(json!({"data":{"actor":"agent","description":"Enrolled."}})),
        &[("idempotency-key", "enroll_requirement"), ("if-match", &etag)],
    )
    .await;
    assert_eq!(status, 200, "{saved}");
    saved["data"]["edit"]["revision"].as_str().unwrap().to_owned()
}

async fn submit(host: &StatementHost) -> String {
    let revision = enroll(host).await;
    let body = json!({"data":{
        "actor":"agent", "declared_by":null, "title":"Review",
        "summary":"Review the saved Requirement.", "confidence":null,
        "source_ids":[], "evidence_references":[], "builds_on":[],
        "expected_revision":revision, "revises":null
    }});
    let (status, value, _) = call(
        host,
        "POST",
        "/requirements/req_shared/submit",
        Some(body),
        &[],
    )
    .await;
    assert_eq!(status, 200, "{value}");
    let proposal = value["data"]["proposal_id"]
        .as_str()
        .expect("submission returns its proposal identity")
        .to_owned();
    assert_eq!(value["data"]["proposal_key"], proposal);
    proposal
}

fn allow_reviewer(repo: &Repository) {
    let mut manifest: Manifest =
        serde_json::from_slice(&std::fs::read(repo.layout.manifest_path()).unwrap()).unwrap();
    manifest.disposition_actor_ids.push("reviewer".into());
    std::fs::write(
        repo.layout.manifest_path(),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
}

#[tokio::test]
async fn stale_requirement_edit_returns_the_current_etag() {
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&repo);
    let (_, _, old_etag) = call(&host, "GET", "/requirements/req_shared", None, &[]).await;
    let old_etag = old_etag.unwrap();
    let body = |description: &str| {
        Some(json!({"data":{"actor":"agent","description":description}}))
    };
    let (status, current, _) = call(
        &host,
        "PATCH",
        "/requirements/req_shared",
        body("Current edit."),
        &[("idempotency-key", "edit-current"), ("if-match", &old_etag)],
    )
    .await;
    assert_eq!(status, 200, "{current}");
    let (status, conflict, _) = call(
        &host,
        "PATCH",
        "/requirements/req_shared",
        body("Stale edit."),
        &[("idempotency-key", "edit-stale"), ("if-match", &old_etag)],
    )
    .await;
    assert_eq!(status, 409, "{conflict}");
    assert_eq!(conflict["error"]["kind"], "requirement_edit_conflict");
    assert_eq!(conflict["error"]["current_etag"], current["data"]["edit"]["etag"]);
}

#[tokio::test]
async fn review_decide_dispatches_and_reports_a_repeated_decision() {
    let repo = Repository::new("The shared graph is readable.");
    allow_reviewer(&repo);
    let host = host(&repo);
    let proposal = submit(&host).await;
    create(
        &host,
        "/requirements",
        Some("create_req_other"),
        json!({"actor":"agent","id":"req_other","statement":"The other record exists.",
            "status":"active","depends_on":[],"supersedes":[]}),
    )
    .await;
    let decision = json!({"data":{
        "actor":{"identity_type":"human","id":"reviewer"},
        "decision":"rejected", "rationale":"The revision needs work.",
        "canonical_artifact":null, "feedback":null, "declared_by":null
    }});
    let (wrong_status, wrong, _) = call(
        &host,
        "POST",
        &format!("/requirements/req_other/submissions/{proposal}/decide"),
        Some(decision.clone()),
        &[],
    )
    .await;
    assert_eq!(wrong_status, 400, "{wrong}");
    assert_eq!(wrong["error"]["kind"], "invalid_update");
    let path = format!("/requirements/req_shared/submissions/{proposal}/decide");
    let (status, value, _) = call(&host, "POST", &path, Some(decision), &[]).await;
    assert_eq!(status, 200, "{value}");
    assert_eq!(value["data"]["fact"], "decided");

    let (status, conflict, _) = call(
        &host,
        "POST",
        &path,
        Some(json!({"data":{
            "actor":{"identity_type":"human","id":"reviewer"},
            "decision":"accepted", "rationale":null,
            "canonical_artifact":{"artifact_type":"requirement","artifact_id":"req_shared"},
            "feedback":null, "declared_by":null
        }})),
        &[],
    )
    .await;
    assert_eq!(status, 409, "{conflict}");
    assert_eq!(conflict["error"]["kind"], "review_submission_conflict");
    assert!(conflict["error"]["current_submission"].is_null());
    assert!(conflict["error"]["current_revision"].is_string());
}

#[tokio::test]
async fn review_withdraw_dispatches_for_a_real_submission() {
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&repo);
    let proposal = submit(&host).await;
    let (status, value, _) = call(
        &host,
        "POST",
        &format!("/requirements/req_shared/submissions/{proposal}/withdraw"),
        Some(json!({"data":{"actor":"agent","declared_by":null,"reason":"Revise it."}})),
        &[],
    )
    .await;
    assert_eq!(status, 200, "{value}");
    assert_eq!(value["data"]["fact"], "withdrawn");
    assert_eq!(value["data"]["requirement_id"], "req_shared");
}
