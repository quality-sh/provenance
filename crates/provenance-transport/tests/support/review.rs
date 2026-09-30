use super::*;
use provenance_core::Manifest;

async fn enroll(host: &StatementHost) -> (String, String) {
    let (status, read, etag) = call(host, "GET", "/requirements/req_shared", None, &[]).await;
    assert_eq!(status, 200, "{read}");
    let etag = etag.unwrap();
    let (status, saved, _) = call(
        host,
        "PATCH",
        "/requirements/req_shared",
        Some(json!({"data":{"actor":"agent","description":"Enrolled."}})),
        &[("if-match", &etag)],
    )
    .await;
    assert_eq!(status, 200, "{saved}");
    let revision = saved["data"]["edit"]["revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let proposal = saved["data"]["decision"]["pending"]["proposal_id"]
        .as_str()
        .expect("the edit response returns its submission identity")
        .to_owned();
    assert_eq!(saved["data"]["decision"]["pending"]["revision"], revision);
    (revision, proposal)
}

async fn submit(host: &StatementHost) -> String {
    enroll(host).await.1
}

async fn submit_at(host: &StatementHost, revision: &str) -> String {
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

async fn edit(host: &StatementHost, _key: &str, description: &str) -> (String, String) {
    let (_, _, etag) = call(host, "GET", "/requirements/req_shared", None, &[]).await;
    let (status, saved, _) = call(
        host,
        "PATCH",
        "/requirements/req_shared",
        Some(json!({"data":{"actor":"agent","description":description}})),
        &[("if-match", &etag.unwrap())],
    )
    .await;
    assert_eq!(status, 200, "{saved}");
    let revision = saved["data"]["edit"]["revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let proposal = saved["data"]["decision"]["pending"]["proposal_id"]
        .as_str()
        .expect("the edit response returns its submission identity")
        .to_owned();
    assert_eq!(saved["data"]["decision"]["pending"]["revision"], revision);
    (revision, proposal)
}

fn conflict(submission: Option<&str>, revision: &str) -> Value {
    json!({"error":{
        "kind":"review_submission_conflict",
        "current_submission":submission,
        "current_revision":revision
    },"meta":{}})
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
    let body =
        |description: &str| Some(json!({"data":{"actor":"agent","description":description}}));
    let (status, current, _) = call(
        &host,
        "PATCH",
        "/requirements/req_shared",
        body("Current edit."),
        &[("if-match", &old_etag)],
    )
    .await;
    assert_eq!(status, 200, "{current}");
    let (status, conflict, _) = call(
        &host,
        "PATCH",
        "/requirements/req_shared",
        body("Stale edit."),
        &[("if-match", &old_etag)],
    )
    .await;
    assert_eq!(status, 409, "{conflict}");
    assert_eq!(conflict["error"]["kind"], "requirement_edit_conflict");
    assert_eq!(
        conflict["error"]["current_etag"],
        current["data"]["edit"]["etag"]
    );
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
        None,
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

#[tokio::test]
async fn submit_conflicts_return_the_typed_http_envelope() {
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&repo);
    let (revision_1, _) = enroll(&host).await;
    let (revision_2, automatic) = edit(&host, "edit-current", "Current revision.").await;
    let body = |revision: &str| {
        json!({"data":{
            "actor":"agent", "declared_by":null, "title":"Review", "summary":"Review it.",
            "confidence":null, "source_ids":[], "evidence_references":[], "builds_on":[],
            "expected_revision":revision, "revises":null
        }})
    };
    let (status, stale, _) = call(
        &host,
        "POST",
        "/requirements/req_shared/submit",
        Some(body(&revision_1)),
        &[],
    )
    .await;
    assert_eq!(status, 409);
    assert_eq!(stale, conflict(Some(&automatic), &revision_2));

    let (status, repeated, _) = call(
        &host,
        "POST",
        "/requirements/req_shared/submit",
        Some(body(&revision_2)),
        &[],
    )
    .await;
    assert_eq!(status, 409);
    assert_eq!(repeated, conflict(Some(&automatic), &revision_2));

    let path = format!("/requirements/req_shared/submissions/{automatic}/withdraw");
    assert_eq!(
        call(
            &host,
            "POST",
            &path,
            Some(json!({"data":{"actor":"agent","declared_by":null,"reason":null}})),
            &[],
        )
        .await
        .0,
        200
    );
    let proposal = submit_at(&host, &revision_2).await;
    assert_ne!(proposal, automatic);
}

async fn assert_terminal_conflicts(
    host: &StatementHost,
    proposal: &str,
    current: Option<&str>,
    revision: &str,
) {
    let paths = [
        (
            "decide",
            json!({"data":{"actor":{"identity_type":"human","id":"reviewer"},
            "decision":"accepted", "rationale":null,
            "canonical_artifact":{"artifact_type":"requirement","artifact_id":"req_shared"},
            "feedback":null, "declared_by":null}}),
        ),
        (
            "withdraw",
            json!({"data":{"actor":"agent","declared_by":null,"reason":null}}),
        ),
    ];
    for (action, body) in paths {
        let path = format!("/requirements/req_shared/submissions/{proposal}/{action}");
        let (status, value, _) = call(host, "POST", &path, Some(body), &[]).await;
        assert_eq!(status, 409, "{value}");
        assert_eq!(value, conflict(current, revision));
    }
}

async fn current_revision(host: &StatementHost) -> String {
    let (status, value, _) = call(host, "GET", "/requirements/req_shared", None, &[]).await;
    assert_eq!(status, 200, "{value}");
    value["data"]["edit"]["revision"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[tokio::test]
async fn terminal_review_conflicts_share_the_http_envelope() {
    let stale_repo = Repository::new("The shared graph is readable.");
    allow_reviewer(&stale_repo);
    let stale_host = host(&stale_repo);
    let stale = submit(&stale_host).await;
    let (stale_revision, current) = edit(&stale_host, "supersede", "Superseded.").await;
    assert_terminal_conflicts(&stale_host, &stale, Some(&current), &stale_revision).await;

    let withdrawn_repo = Repository::new("The shared graph is readable.");
    allow_reviewer(&withdrawn_repo);
    let withdrawn_host = host(&withdrawn_repo);
    let withdrawn = submit(&withdrawn_host).await;
    let revision = current_revision(&withdrawn_host).await;
    let path = format!("/requirements/req_shared/submissions/{withdrawn}/withdraw");
    assert_eq!(
        call(
            &withdrawn_host,
            "POST",
            &path,
            Some(json!({"data":{
                "actor":"agent","declared_by":null,"reason":null
            }})),
            &[]
        )
        .await
        .0,
        200
    );
    assert_terminal_conflicts(&withdrawn_host, &withdrawn, None, &revision).await;

    let decided_repo = Repository::new("The shared graph is readable.");
    allow_reviewer(&decided_repo);
    let decided_host = host(&decided_repo);
    let decided = submit(&decided_host).await;
    let revision = current_revision(&decided_host).await;
    let path = format!("/requirements/req_shared/submissions/{decided}/decide");
    assert_eq!(
        call(
            &decided_host,
            "POST",
            &path,
            Some(json!({"data":{
                "actor":{"identity_type":"human","id":"reviewer"}, "decision":"rejected",
                "rationale":"Needs work.", "canonical_artifact":null,
                "feedback":null, "declared_by":null
            }})),
            &[]
        )
        .await
        .0,
        200
    );
    assert_terminal_conflicts(&decided_host, &decided, None, &revision).await;
}
