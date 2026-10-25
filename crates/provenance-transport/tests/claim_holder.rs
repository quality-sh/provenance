//! A shaping claim has one holder; another actor cannot take or drop it.
#![cfg(feature = "test-fixture")]
mod support {
    pub mod records;
    pub mod resource_http;
}
use provenance_macros::verifies;
use serde_json::{json, Value};
use support::records::Repository;
use support::resource_http::{call, host};

async fn act(
    host: &provenance_transport::StatementHost,
    record: &str,
    action: &str,
    actor: &str,
) -> (u16, Value) {
    let path = format!("{record}/{action}");
    call(host, "POST", &path, Some(json!({"data":{"actor":actor}}))).await
}

#[tokio::test]
#[verifies("rule_claim_holder_exclusive", examples)]
async fn a_claim_has_one_holder_and_only_the_holder_releases_it() {
    let repo = Repository::new("The shared graph is readable.");
    repo.all_kinds();
    let host = host(&repo, true);
    for record in ["/topics/topic_shared", "/questions/question_shared"] {
        let (status, claimed) = act(&host, record, "claim", "alice").await;
        assert_eq!(status, 200, "{record}: {claimed}");
        assert_eq!(claimed["data"]["claimed_by"], "alice");
        let claimed_at = claimed["data"]["claimed_at"].clone();

        let conflict = json!({"kind":"claim_conflict", "holder":"alice"});
        let (status, refused) = act(&host, record, "claim", "bob").await;
        assert_eq!(status, 409, "{record}: {refused}");
        assert_eq!(refused["error"], conflict);

        let (status, again) = act(&host, record, "claim", "alice").await;
        assert_eq!(status, 200, "{record}: {again}");
        assert_eq!(again["data"]["claimed_by"], "alice");
        assert_eq!(again["data"]["claimed_at"], claimed_at, "{record}");

        let (status, refused) = act(&host, record, "release", "bob").await;
        assert_eq!(status, 409, "{record}: {refused}");
        assert_eq!(refused["error"], conflict);

        let (status, read) = call(&host, "GET", record, None).await;
        assert_eq!(status, 200, "{read}");
        assert_eq!(read["data"]["claimed_by"], "alice", "{record}");

        let (status, released) = act(&host, record, "release", "alice").await;
        assert_eq!(status, 200, "{record}: {released}");
        assert!(released["data"]["claimed_by"].is_null(), "{released}");

        let (status, unclaimed) = act(&host, record, "release", "alice").await;
        assert_eq!(status, 400, "{record}: {unclaimed}");
        assert_eq!(unclaimed["error"]["kind"], "invalid_update");
    }
}

#[tokio::test]
#[verifies("rule_write_actor_required", examples)]
async fn a_release_names_its_actor() {
    let repo = Repository::new("The shared graph is readable.");
    repo.all_kinds();
    let host = host(&repo, true);
    let (status, refused) = act(&host, "/topics/topic_shared", "claim", "  ").await;
    assert_eq!(status, 400, "{refused}");
    assert_eq!(
        refused["error"],
        json!({"kind":"invalid_input", "field":"request.actor", "reason":"invalid_value"})
    );
    act(&host, "/topics/topic_shared", "claim", "alice").await;
    let (status, refused) = call(
        &host,
        "POST",
        "/topics/topic_shared/release",
        Some(json!({"data":{}})),
    )
    .await;
    assert_eq!(status, 400, "{refused}");
    assert_eq!(
        refused["error"],
        json!({"kind":"invalid_input", "field":"request.actor", "reason":"required"})
    );
    let (status, read) = call(&host, "GET", "/topics/topic_shared", None).await;
    assert_eq!(status, 200, "{read}");
    assert_eq!(read["data"]["claimed_by"], "alice");
}
