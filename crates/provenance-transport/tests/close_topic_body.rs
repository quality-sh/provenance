//! The request body that `POST /topics/{id}/close` accepts.
#![cfg(feature = "test-fixture")]
mod support {
    pub mod records;
    pub mod resource_http;
}
use serde_json::json;
use support::records::Repository;
use support::resource_http::{call, host};

const CLOSE: &str = "/topics/topic_shared/close";

async fn topic_status(host: &provenance_transport::StatementHost) -> serde_json::Value {
    let (status, read) = call(host, "GET", "/topics/topic_shared", None).await;
    assert_eq!(status, 200, "{read}");
    read["data"]["status"].clone()
}

// No Rule: this test pins the body of the close route before a contract change.
#[tokio::test]
async fn close_topic_requires_a_data_object_with_no_field() {
    let repo = Repository::new("The shared graph is readable.");
    repo.all_kinds();
    let host = host(&repo, true);
    let open = topic_status(&host).await;

    let (status, refused) = call(&host, "POST", CLOSE, None).await;
    assert_eq!(status, 400, "{refused}");
    assert_eq!(
        refused["error"],
        json!({"kind":"invalid_input","field":null,"reason":"malformed_json"})
    );
    assert_eq!(topic_status(&host).await, open);

    let (status, closed) = call(&host, "POST", CLOSE, Some(json!({"data":{}}))).await;
    assert_eq!(status, 200, "{closed}");
    assert_eq!(closed["data"]["status"], "closed");
}
