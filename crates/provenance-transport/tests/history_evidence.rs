//! Evidence reads for a field that one version does not hold, the refusals of
//! an address that names no evidence, and the native limit refusal.
#![cfg(feature = "test-fixture")]

mod support {
    pub mod records;
    pub mod resource_http;
}

use provenance_core::{ScopeId, StableId};
use provenance_macros::verifies;
use provenance_store::review::{CreateReviewRequirement, SaveRequirement};
use serde_json::json;
use support::records::Repository;
use support::resource_http::{access, call, host};

fn sid(value: &str) -> StableId {
    StableId::new(value).unwrap()
}

fn scope() -> ScopeId {
    ScopeId::new("default").unwrap()
}

/// Commits the saved graph state and returns the commit id.
fn commit(repo: &Repository) -> String {
    let git = |args: &[&str]| {
        let output = provenance_store::git_process::git()
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

/// A Requirement committed without a description, then committed with one. Returns
/// the repository and the two commit ids.
fn repository_with_description_change() -> (Repository, String, String) {
    let repo = Repository::new("The shared graph is readable.");
    let store = repo.open().store();
    let create: CreateReviewRequirement = serde_json::from_value(json!({
        "actor":"ben","origin":null,
        "create":{"scope_id":"default","id":"req_described",
            "statement":"The system stores each record.",
            "status":"discovery","depends_on":[],"supersedes":[]}
    }))
    .unwrap();
    store.create_review_requirement(create).unwrap();
    let created = commit(&repo);
    let save: SaveRequirement = serde_json::from_value(json!({
        "actor":"ben",
        "expected_etag":store
            .requirement_edit_state(&scope(), &sid("req_described"))
            .unwrap()
            .etag,
        "update":{"scope_id":"default","id":"req_described","description":"Shared detail"},
        "relationships":null
    }))
    .unwrap();
    store.save_requirement(save).unwrap();
    let assigned = commit(&repo);
    (repo, created, assigned)
}

#[tokio::test]
#[verifies("rule_evidence_read_states_absent_fields", examples)]
async fn before_evidence_of_an_added_field_is_null() {
    let (repo, created, assigned) = repository_with_description_change();
    let host = host(&repo, false);
    let base = format!("/requirements/req_described/history/{assigned}/evidence");

    let (status, after) = call(
        &host,
        "GET",
        &format!("{base}/after?field=description"),
        None,
    )
    .await;
    assert_eq!(status, 200, "{after}");
    assert_eq!(after["data"]["json_text"], "\"Shared detail\"", "{after}");
    let (status, before) = call(
        &host,
        "GET",
        &format!("{base}/before?field=description"),
        None,
    )
    .await;
    assert_eq!(status, 200, "{before}");
    assert_eq!(before["data"]["json_text"], "null", "{before}");
    assert_eq!(before["data"]["version"], created, "{before}");
    assert_eq!(before["data"]["field"], "description", "{before}");
    assert_eq!(before["data"]["offset"], 0, "{before}");
    assert!(before["data"]["next_offset"].is_null(), "{before}");
}

#[tokio::test]
#[verifies("rule_evidence_read_states_absent_fields", examples)]
async fn evidence_refuses_a_missing_before_version_and_an_unknown_field() {
    let (repo, created, assigned) = repository_with_description_change();
    let host = host(&repo, false);
    let base = "/requirements/req_described/history";

    let (status, failure) = call(
        &host,
        "GET",
        &format!("{base}/{created}/evidence/before?field=statement"),
        None,
    )
    .await;
    assert_eq!(status, 404, "{failure}");
    assert_eq!(failure["error"]["kind"], "resource_not_found", "{failure}");
    for side in ["before", "after"] {
        let (status, failure) = call(
            &host,
            "GET",
            &format!("{base}/{assigned}/evidence/{side}?field=nope"),
            None,
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
    let (repo, _, assigned) = repository_with_description_change();
    let host = host(&repo, false);
    let path =
        format!("/requirements/req_described/history/{assigned}/evidence/after?field=description");

    let (status, page) = call(&host, "GET", &format!("{path}&offset=14"), None).await;
    assert_eq!(status, 200, "{page}");
    assert_eq!(page["data"]["json_text"], "\"", "{page}");
    let (status, failure) = call(&host, "GET", &format!("{path}&offset=15"), None).await;
    assert_eq!(status, 400, "{failure}");
    assert_eq!(failure["error"]["kind"], "invalid_input", "{failure}");
    assert_eq!(failure["error"]["field"], "offset", "{failure}");
    assert_eq!(failure["error"]["reason"], "invalid_value", "{failure}");
}

/// Characterization (sgkl): a committed line that holds the removed `domain_id`
/// member. The evidence read gives the old raw value and never a 500.
#[tokio::test]
async fn evidence_of_a_removed_domain_id_member_is_its_old_value() {
    let (repo, _, _) = repository_with_description_change();
    let path = repo
        .dir
        .path()
        .join(".provenance/state/scopes/default/requirements/req.jsonl");
    let original = std::fs::read_to_string(&path).unwrap();
    let with_member = original
        .lines()
        .map(|line| {
            let mut record: serde_json::Value = serde_json::from_str(line).unwrap();
            if record["id"] == "req_described" {
                record["domain_id"] = json!("domain_old");
            }
            record.to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    std::fs::write(&path, with_member).unwrap();
    commit(&repo);
    std::fs::write(&path, original).unwrap();
    let removed = commit(&repo);
    let host = host(&repo, false);
    let base = format!("/requirements/req_described/history/{removed}/evidence");
    let mut answers = Vec::new();
    for side in ["before", "after"] {
        let (status, body) = call(
            &host,
            "GET",
            &format!("{base}/{side}?field=domain_id"),
            None,
        )
        .await;
        answers.push(json!({"side": side, "status": status,
            "json_text": body["data"]["json_text"], "error": body["error"]}));
    }
    assert_eq!(
        json!(answers),
        json!([
            {"side":"before","status":200,"json_text":"\"domain_old\"","error":null},
            {"side":"after","status":200,"json_text":"null","error":null}
        ])
    );
}

// Implementation aid: the native invoke path has no query-string bounds, so
// the handler's own limit check must publish the typed refusal.
#[tokio::test]
async fn native_review_reads_refuse_an_out_of_range_limit() {
    let (repo, _, _) = repository_with_description_change();
    let history = json!({"record_kind":"requirement","record_id":"req_described"});
    let discussions = json!({"parent":{"node_type":"requirement","node_id":"req_described"}});
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
            let failure = provenance_contract::invoke_with(
                operation,
                provenance_core::SDK_PROTOCOL_VERSION,
                call,
                provenance_store::operations::catalog::Direct(std::sync::Arc::new(access(&repo))),
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
