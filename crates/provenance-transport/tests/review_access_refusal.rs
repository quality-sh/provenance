//! A read-only caller cannot write a review submission, decision or withdrawal.
#![cfg(feature = "test-fixture")]
mod support {
    pub mod records;
    pub mod resource_http;
}
use serde_json::json;
use support::records::{allow_reviewer, Repository};
use support::resource_http::{call, call_with_headers, host};

/// Implementation aid: no Rule states write access for review routes. This test
/// keeps each review write route behind the write grant and checks that a refusal
/// leaves the review state unchanged.
#[tokio::test]
async fn review_writes_without_write_access_are_denied_and_change_nothing() {
    let repo = Repository::new("The shared graph is readable.");
    allow_reviewer(&repo);
    let writer = host(&repo, true);
    let (status, read) = call(&writer, "GET", "/requirements/req_shared", None).await;
    assert_eq!(status, 200, "{read}");
    let etag = read["data"]["edit"]["etag"].as_str().unwrap().to_owned();
    let (status, saved) = call_with_headers(
        &writer,
        "PATCH",
        "/requirements/req_shared",
        Some(json!({"data":{"actor":"agent", "description":"Enrolled."}})),
        &[("if-match", &etag)],
    )
    .await;
    assert_eq!(status, 200, "{saved}");
    let proposal = saved["data"]["decision"]["pending"]["proposal_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let reader = host(&repo, false);
    let (status, before) = call(&reader, "GET", "/requirements/req_shared", None).await;
    assert_eq!(status, 200, "{before}");

    let submissions = format!("/requirements/req_shared/submissions/{proposal}");
    for (path, data) in [
        (
            "/requirements/req_shared/submit".to_owned(),
            json!({"actor":"agent", "declared_by":null, "title":"Review",
                "summary":"Review the saved Requirement.", "confidence":null,
                "source_ids":[], "evidence_references":[], "builds_on":[],
                "expected_revision":null, "revises":null}),
        ),
        (
            format!("{submissions}/decide"),
            json!({"actor":{"identity_type":"human","id":"reviewer"},
                "decision":"rejected", "rationale":"Name the scope.",
                "canonical_artifact":null,
                "feedback":{"role":"user","body":"Name the scope."},
                "declared_by":null}),
        ),
        (
            format!("{submissions}/withdraw"),
            json!({"actor":"agent", "declared_by":null, "reason":"Revise it."}),
        ),
    ] {
        let (status, refused) = call(&reader, "POST", &path, Some(json!({ "data": data }))).await;
        assert_eq!(status, 403, "{path}: {refused}");
        assert_eq!(refused["error"], json!({"kind":"access_denied"}), "{path}");
    }

    let (status, after) = call(&reader, "GET", "/requirements/req_shared", None).await;
    assert_eq!(status, 200, "{after}");
    assert_eq!(after, before);
    let (status, discussions) =
        call(&reader, "GET", "/requirements/req_shared/discussions", None).await;
    assert_eq!(status, 200, "{discussions}");
    assert_eq!(discussions["data"]["items"], json!([]));
}
