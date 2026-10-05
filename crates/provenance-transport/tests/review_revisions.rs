#![cfg(feature = "test-fixture")]

mod support {
    #[allow(dead_code)]
    pub mod api_fixture;
    pub mod records;
}

use axum::{body::Body, http::Request};
use provenance_macros::verifies;
use provenance_transport::StatementHost;
use serde_json::{json, Value};
use support::{
    api_fixture::write_host as host,
    records::{allow_reviewer, Repository},
};
use tower::ServiceExt as _;

async fn call(
    host: &StatementHost,
    method: &str,
    path: &str,
    body: Option<Value>,
    etag: Option<&str>,
) -> (u16, Value, Option<String>) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("host", "fixture.test")
        .header("authorization", "Bearer fixture-secret");
    if let Some(etag) = etag {
        request = request.header("if-match", etag);
    }
    if body.is_some() {
        request = request.header("content-type", "application/json");
    }
    let body = body.map_or_else(Body::empty, |value| Body::from(value.to_string()));
    let response = host
        .router()
        .oneshot(request.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status().as_u16();
    let etag = response
        .headers()
        .get("etag")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let value = serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    (status, value, etag)
}

async fn set_statement(host: &StatementHost, statement: &str) -> Value {
    let (status, read, etag) = call(host, "GET", "/requirements/req_flow", None, None).await;
    assert_eq!(status, 200, "{read}");
    let (status, saved, _) = call(
        host,
        "PATCH",
        "/requirements/req_flow",
        Some(json!({"data":{"actor":"agent","statement":statement}})),
        etag.as_deref(),
    )
    .await;
    assert_eq!(status, 200, "{saved}");
    saved["data"].clone()
}

/// Flow: create, accept, change the text to B, then return it to A.
#[tokio::test]
#[verifies("rule_approval_accepts_reviewed_version", examples)]
#[verifies("rule_content_change_opens_review_submission", examples)]
async fn accepted_text_returns_as_accepted() {
    let repo = Repository::new("The shared graph is readable.");
    allow_reviewer(&repo);
    let host = host(&repo);
    let (status, created, _) = call(
        &host,
        "POST",
        "/requirements",
        Some(json!({"data":{
            "actor":"agent","id":"req_flow","statement":"The flow text is A.",
            "status":"discovery","depends_on":[],"supersedes":[]
        }})),
        None,
    )
    .await;
    assert_eq!(status, 200, "{created}");
    let revision_a = created["data"]["edit"]["revision"].clone();
    let proposal = created["data"]["decision"]["pending"]["proposal_id"]
        .as_str()
        .expect("creation opens a review submission")
        .to_owned();
    let (status, decided, _) = call(
        &host,
        "POST",
        &format!("/requirements/req_flow/submissions/{proposal}/decide"),
        Some(json!({"data":{
            "actor":{"identity_type":"human","id":"reviewer"},
            "decision":"accepted", "rationale":null,
            "canonical_artifact":{"artifact_type":"requirement","artifact_id":"req_flow"},
            "feedback":null, "declared_by":null
        }})),
        None,
    )
    .await;
    assert_eq!(status, 200, "{decided}");
    let disposition = decided["data"]["disposition_id"].clone();
    let (status, accepted, _) = call(&host, "GET", "/requirements/req_flow", None, None).await;
    assert_eq!(status, 200, "{accepted}");
    assert_eq!(accepted["data"]["status"], "discovery");

    let changed = set_statement(&host, "The flow text is B.").await;
    assert_ne!(changed["edit"]["revision"], revision_a);
    assert!(changed["decision"]["current_acceptance"].is_null());
    assert!(changed["decision"]["pending"]["proposal_id"].is_string());

    let returned = set_statement(&host, "The flow text is A.").await;
    assert_eq!(returned["status"], "discovery");
    assert_eq!(returned["edit"]["revision"], revision_a);
    assert!(returned["decision"]["pending"].is_null(), "{returned}");
    let acceptance = &returned["decision"]["current_acceptance"];
    assert_eq!(acceptance["revision"], revision_a);
    assert_eq!(acceptance["disposition"]["id"], disposition);
}

fn commit(repo: &Repository) -> String {
    let git = |args: &[&str]| {
        let output = std::process::Command::new("git")
            .args(args)
            .env("GIT_AUTHOR_NAME", "Reviewer")
            .env("GIT_AUTHOR_EMAIL", "reviewer@example.com")
            .env("GIT_COMMITTER_NAME", "Reviewer")
            .env("GIT_COMMITTER_EMAIL", "reviewer@example.com")
            .current_dir(repo.dir.path())
            .output()
            .unwrap();
        assert!(output.status.success(), "git {args:?}: {output:?}");
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    };
    if !repo.dir.path().join(".git").exists() {
        git(&["init", "-q"]);
    }
    git(&["add", ".provenance/state"]);
    git(&["-c", "commit.gpgsign=false", "commit", "-q", "-m", "Save record"]);
    git(&["rev-parse", "HEAD"])
}

#[tokio::test]
#[verifies("rule_record_history_reads_git", examples)]
#[verifies("rule_change_view_has_before_and_after_states", examples)]
async fn rejected_text_return_uses_previous_git_version_for_evidence() {
    let repo = Repository::new("The shared graph is readable.");
    allow_reviewer(&repo);
    let host = host(&repo);
    let (status, created, _) = call(
        &host,
        "POST",
        "/requirements",
        Some(json!({"data":{
            "actor":"agent","id":"req_flow","statement":"The limit is 2000 dollars.",
            "status":"discovery","depends_on":[],"supersedes":[]
        }})),
        None,
    )
    .await;
    assert_eq!(status, 200, "{created}");
    let revision_a = created["data"]["edit"]["revision"].clone();
    let original = commit(&repo);
    let changed = set_statement(&host, "The limit is 3000 dollars.").await;
    let previous = commit(&repo);
    let proposal = changed["decision"]["pending"]["proposal_id"].as_str().unwrap();
    let (status, rejected, _) = call(
        &host,
        "POST",
        &format!("/requirements/req_flow/submissions/{proposal}/decide"),
        Some(json!({"data":{
            "actor":{"identity_type":"human","id":"reviewer"},
            "decision":"rejected", "rationale":"Keep the limit at 2000 dollars.",
            "canonical_artifact":null, "feedback":null, "declared_by":null
        }})),
        None,
    )
    .await;
    assert_eq!(status, 200, "{rejected}");
    let returned = set_statement(&host, "The limit is 2000 dollars.").await;
    assert_eq!(returned["decision"]["pending"]["revision"], revision_a);
    assert_ne!(returned["decision"]["pending"]["proposal_id"], proposal);

    for committed in [false, true] {
        let selected = if committed {
            commit(&repo)
        } else {
            "working".to_owned()
        };
        let base = "/requirements/req_flow/history";
        let (status, history, _) = call(&host, "GET", base, None, None).await;
        assert_eq!(status, 200, "{history}");
        let entries = history["data"]["items"].as_array().unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0]["id"], original);
        let latest = entries.last().unwrap();
        assert_eq!(latest["id"], selected);
        assert_eq!(latest["revision"], revision_a);
        assert_eq!(latest["before"], previous);
        assert_eq!(latest["changed_fields"], json!(["statement"]));
        let (status, entry, _) =
            call(&host, "GET", &format!("{base}/{selected}"), None, None).await;
        assert_eq!(status, 200, "{entry}");
        assert_eq!(entry["data"], *latest);
        for (side, expected) in [
            ("before", "The limit is 3000 dollars."),
            ("after", "The limit is 2000 dollars."),
        ] {
            let path = format!("{base}/{selected}/evidence/{side}?field=statement");
            let (status, evidence, _) = call(&host, "GET", &path, None, None).await;
            assert_eq!(status, 200, "{evidence}");
            let text: String =
                serde_json::from_str(evidence["data"]["json_text"].as_str().unwrap()).unwrap();
            assert_eq!(text, expected);
        }
    }
}
