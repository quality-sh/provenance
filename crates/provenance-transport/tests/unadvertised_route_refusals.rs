//! Characterization with no Rule: the router answer to an operation that the host
//! access does not advertise. A denied operation and a host with no access give the
//! same answer.
#![cfg(feature = "test-fixture")]

use crate::support::{records::Repository, resource_http};
use provenance_transport::StatementHost;
use serde_json::{json, Value};

#[tokio::test]
async fn an_unadvertised_route_answers_access_denied() {
    let repo = Repository::new("The shared graph is readable.");
    let denied = StatementHost::with_fixture_access(
        resource_http::access(&repo)
            .allow_writes()
            .deny_operation("get-requirement"),
    );
    let mut rows = Vec::new();
    for (name, host) in [
        ("read on a host that denies get-requirement", denied),
        ("read on a host with no access", StatementHost::default()),
    ] {
        let (status, body) =
            resource_http::call(&host, "GET", "/requirements/req_shared", None).await;
        rows.push(json!({"row": name, "status": status, "body": body}));
        host.shutdown().await;
    }
    assert_eq!(
        Value::Array(rows),
        json!([
            {"row": "read on a host that denies get-requirement", "status": 403,
             "body": {"error": {"kind": "access_denied"}, "meta": {}}},
            {"row": "read on a host with no access", "status": 403,
             "body": {"error": {"kind": "access_denied"}, "meta": {}}}
        ])
    );
}
