//! Evidence reads for a field that one version does not hold, the refusals of
//! an address that names no evidence, and the native limit refusal.
#![cfg(feature = "test-fixture")]

mod support {
    pub mod records;
}

use axum::{body::Body, http::Request};
use provenance_core::{ScopeId, StableId};
use provenance_macros::verifies;
use provenance_store::{
    review::{CreateReviewRequirement, SaveRequirement},
    state_store::StateStore,
};
use provenance_transport::{
    fixture::{FixtureAccess, Target},
    StatementHost,
};
use serde_json::{json, Value};
use support::records::Repository;
use tower::ServiceExt as _;

fn access(repo: &Repository) -> FixtureAccess {
    FixtureAccess::new(
        vec![Target {
            id: "selected".into(),
            root: repo.dir.path().to_path_buf(),
        }],
        vec![("selected".into(), "default".into())],
        "fixture-secret",
        "fixture.test",
    )
    .unwrap()
}

fn host(repo: &Repository) -> StatementHost {
    StatementHost::with_fixture_access(access(repo))
}

async fn get(host: &StatementHost, path: &str) -> (u16, Value) {
    let request = Request::builder()
        .method("GET")
        .uri(path)
        .header("host", "fixture.test")
        .header("authorization", "Bearer fixture-secret")
        .body(Body::empty())
        .unwrap();
    let response = host.router().oneshot(request).await.unwrap();
    let status = response.status().as_u16();
    let value = serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    (status, value)
}

fn sid(value: &str) -> StableId {
    StableId::new(value).unwrap()
}

fn scope() -> ScopeId {
    ScopeId::new("default").unwrap()
}

/// Commits the saved graph state and returns the commit id.
fn commit(repo: &Repository) -> String {
    let git = |args: &[&str]| {
        let output = std::process::Command::new("git")
            .args([
                "-c",
                "user.name=Reviewer",
                "-c",
                "user.email=reviewer@example.com",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .current_dir(repo.dir.path())
            .output()
            .unwrap();
        assert!(output.status.success(), "git {args:?} failed: {output:?}");
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    };
    if !repo.dir.path().join(".git").exists() {
        git(&["init", "-q"]);
    }
    git(&["add", ".provenance/state"]);
    git(&["commit", "-q", "-m", "Save"]);
    git(&["rev-parse", "HEAD"])
}

/// A Requirement committed without a domain, then committed with one. Returns
/// the repository and the two commit ids.
fn repository_with_domain_change() -> (Repository, String, String) {
    let repo = Repository::new("The shared graph is readable.");
    let store = StateStore::new(repo.layout.clone());
    let create: CreateReviewRequirement = serde_json::from_value(json!({
        "actor":"ben","origin":null,
        "create":{"scope_id":"default","id":"req_domain",
            "statement":"The system stores each record.",
            "status":"discovery","depends_on":[],"supersedes":[]}
    }))
    .unwrap();
    store.create_review_requirement(create).unwrap();
    let created = commit(&repo);
    let save: SaveRequirement = serde_json::from_value(json!({
        "actor":"ben",
        "expected_etag":store
            .requirement_edit_state(&scope(), &sid("req_domain"))
            .unwrap()
            .etag,
        "update":{"scope_id":"default","id":"req_domain","domain_id":"domain_shared"},
        "relationships":null
    }))
    .unwrap();
    store.save_requirement(save).unwrap();
    let assigned = commit(&repo);
    (repo, created, assigned)
}

#[tokio::test]
#[verifies("rule_evidence_read_states_absent_fields", examples)]
#[verifies("rule_change_view_has_before_and_after_states", examples)]
async fn before_evidence_of_an_added_field_is_null() {
    let (repo, created, assigned) = repository_with_domain_change();
    let host = host(&repo);
    let base = format!("/requirements/req_domain/history/{assigned}/evidence");

    let (status, after) = get(&host, &format!("{base}/after?field=domain_id")).await;
    assert_eq!(status, 200, "{after}");
    assert_eq!(after["data"]["json_text"], "\"domain_shared\"", "{after}");
    let (status, before) = get(&host, &format!("{base}/before?field=domain_id")).await;
    assert_eq!(status, 200, "{before}");
    assert_eq!(before["data"]["json_text"], "null", "{before}");
    assert_eq!(before["data"]["version"], created, "{before}");
    assert_eq!(before["data"]["field"], "domain_id", "{before}");
    assert_eq!(before["data"]["offset"], 0, "{before}");
    assert!(before["data"]["next_offset"].is_null(), "{before}");
}

#[tokio::test]
#[verifies("rule_evidence_read_states_absent_fields", examples)]
async fn evidence_refuses_a_missing_before_version_and_an_unknown_field() {
    let (repo, created, assigned) = repository_with_domain_change();
    let host = host(&repo);
    let base = "/requirements/req_domain/history";

    let (status, failure) = get(
        &host,
        &format!("{base}/{created}/evidence/before?field=statement"),
    )
    .await;
    assert_eq!(status, 404, "{failure}");
    assert_eq!(failure["error"]["kind"], "resource_not_found", "{failure}");
    for side in ["before", "after"] {
        let (status, failure) = get(
            &host,
            &format!("{base}/{assigned}/evidence/{side}?field=nope"),
        )
        .await;
        assert_eq!(status, 404, "{side}: {failure}");
        assert_eq!(
            failure["error"]["kind"], "resource_not_found",
            "{side}: {failure}"
        );
    }
}

#[tokio::test]
#[verifies("rule_evidence_read_states_absent_fields", examples)]
async fn evidence_refuses_an_offset_outside_the_selected_text() {
    let (repo, _, assigned) = repository_with_domain_change();
    let host = host(&repo);
    let path =
        format!("/requirements/req_domain/history/{assigned}/evidence/after?field=domain_id");

    let (status, page) = get(&host, &format!("{path}&offset=14")).await;
    assert_eq!(status, 200, "{page}");
    assert_eq!(page["data"]["json_text"], "\"", "{page}");
    let (status, failure) = get(&host, &format!("{path}&offset=15")).await;
    assert_eq!(status, 400, "{failure}");
    assert_eq!(failure["error"]["kind"], "invalid_input", "{failure}");
    assert_eq!(failure["error"]["field"], "offset", "{failure}");
    assert_eq!(failure["error"]["reason"], "invalid_value", "{failure}");
}

// Implementation aid: the native invoke path has no query-string bounds, so
// the handler's own limit check must publish the typed refusal.
#[tokio::test]
async fn native_review_reads_refuse_an_out_of_range_limit() {
    let (repo, _, _) = repository_with_domain_change();
    let history = json!({"record_kind":"requirement","record_id":"req_domain"});
    let discussions = json!({"parent":{"node_type":"requirement","node_id":"req_domain"}});
    for (operation, request) in [
        ("review-history", history),
        ("review-discussions", discussions),
    ] {
        for limit in [0, 201] {
            let mut request = request.clone();
            request["limit"] = json!(limit);
            let call = json!({
                "context":{"repository":"selected","scope":"default"},
                "request":request
            });
            let failure = provenance_store::operations::catalog::invoke_with(
                operation,
                provenance_core::SDK_PROTOCOL_VERSION,
                call,
                std::sync::Arc::new(access(&repo)),
            )
            .await
            .unwrap_err();
            assert_eq!(
                failure.error,
                json!({"kind":"invalid_input","field":"limit","reason":"invalid_value"}),
                "{operation} with limit {limit}"
            );
        }
    }
}
