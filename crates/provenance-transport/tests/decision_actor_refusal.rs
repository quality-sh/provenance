//! A decision by an actor outside the allowlist is an authorization refusal.
#![cfg(feature = "test-fixture")]
mod support {
    pub mod records;
    pub mod resource_http;
}
use provenance_macros::verifies;
use serde_json::{json, Value};
use support::records::{allow_reviewer, Repository};
use support::resource_http::{call, call_with_headers, host};

fn decision(actor: &str) -> Value {
    json!({"data":{
        "actor":{"identity_type":"human", "id":actor},
        "decision":"rejected", "rationale":"The revision needs work.",
        "canonical_artifact":null, "feedback":null, "declared_by":null
    }})
}

#[tokio::test]
#[verifies("rule_disposition_actor_allowlist", examples)]
async fn an_actor_outside_the_allowlist_gets_access_denied() {
    let repo = Repository::new("The shared graph is readable.");
    allow_reviewer(&repo);
    let host = host(&repo, true);
    let (status, read) = call(&host, "GET", "/requirements/req_shared", None).await;
    assert_eq!(status, 200, "{read}");
    let etag = read["data"]["edit"]["etag"].as_str().unwrap().to_owned();
    let (status, saved) = call_with_headers(
        &host,
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
    let path = format!("/requirements/req_shared/submissions/{proposal}/decide");

    let (status, refused) = call(&host, "POST", &path, Some(decision("intruder"))).await;
    assert_eq!(status, 403, "{refused}");
    assert_eq!(refused["error"], json!({"kind":"access_denied"}));

    let (status, read) = call(&host, "GET", "/requirements/req_shared", None).await;
    assert_eq!(status, 200, "{read}");
    assert_eq!(read["data"]["decision"]["pending"]["proposal_id"], proposal);

    let (status, decided) = call(&host, "POST", &path, Some(decision("reviewer"))).await;
    assert_eq!(status, 200, "{decided}");
    assert_eq!(decided["data"]["fact"], "decided");
}
