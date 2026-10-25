#![cfg(feature = "test-fixture")]
//! A request body that stalls cannot hold its ingress slot past the request deadline.

mod support {
    pub mod records;
    pub mod resource_http;
}

use axum::{body::Body, http::Request};
use futures::{poll, stream, StreamExt as _};
use provenance_macros::verifies;
use provenance_transport::StatementHost;
use serde_json::Value;
use std::{convert::Infallible, future::Future, pin::Pin, time::Duration};
use support::{
    records::Repository,
    resource_http::{call, host},
};
use tower::ServiceExt as _;

type Reply = Pin<Box<dyn Future<Output = (u16, Value)> + Send>>;

/// A request whose body sends one byte and then never completes.
fn stalled(host: &StatementHost, method: &str, path: &str) -> Reply {
    let body = Body::from_stream(
        stream::once(async { Ok::<_, Infallible>("{") }).chain(stream::pending()),
    );
    let request = Request::builder()
        .method(method)
        .uri(path)
        .header("host", "fixture.test")
        .header("authorization", "Bearer fixture-secret")
        .header("content-type", "application/json")
        .body(body)
        .unwrap();
    let router = host.router();
    Box::pin(async move {
        let response = router.oneshot(request).await.unwrap();
        let status = response.status().as_u16();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, serde_json::from_slice(&bytes).unwrap())
    })
}

#[verifies("rule_host_body_read_bound", examples)]
#[tokio::test(start_paused = true)]
async fn stalled_bodies_are_refused_at_the_request_deadline_and_free_their_slots() {
    let repo = Repository::new("The shared graph is readable.");
    repo.all_kinds();
    let host = host(&repo, true);
    let started = tokio::time::Instant::now();
    let mut writes: Vec<Reply> = (0..4)
        .map(|_| stalled(&host, "POST", "/requirements/req_shared/discussions"))
        .collect();
    let mut reads: Vec<Reply> = (0..4)
        .map(|_| stalled(&host, "GET", "/requirements/req_shared"))
        .collect();
    for request in writes.iter_mut().chain(&mut reads) {
        assert!(poll!(request).is_pending());
    }
    tokio::time::advance(Duration::from_millis(2999)).await;
    for request in writes.iter_mut().chain(&mut reads) {
        assert!(
            poll!(request).is_pending(),
            "the body read lasts to the deadline"
        );
    }
    tokio::time::advance(Duration::from_millis(1)).await;
    for request in writes.into_iter().chain(reads) {
        let (status, body) = tokio::time::timeout(Duration::ZERO, request)
            .await
            .expect("the body read ends at the request deadline");
        assert_eq!(status, 400, "{body}");
        assert_eq!(body["error"]["kind"], "invalid_input", "{body}");
        assert_eq!(body["error"]["reason"], "incomplete", "{body}");
        assert_eq!(body["error"]["field"], Value::Null, "{body}");
    }
    assert_eq!(started.elapsed(), Duration::from_secs(3));
    let (status, body) = call(&host, "GET", "/requirements/req_shared", None).await;
    assert_eq!(status, 200, "{body}");
    host.shutdown().await;
}

#[verifies("rule_host_body_read_bound", examples)]
#[tokio::test(start_paused = true)]
async fn a_body_that_completes_before_the_deadline_is_accepted() {
    let repo = Repository::new("The shared graph is readable.");
    repo.all_kinds();
    let host = host(&repo, true);
    let (release, wait) = tokio::sync::oneshot::channel();
    let body = Body::from_stream(stream::once(async move {
        let _ = wait.await;
        Ok::<_, Infallible>(
            serde_json::json!({"data": {
                "actor":"reviewer", "declared_by":null, "role":"user", "body":"Check the body."
            }})
            .to_string(),
        )
    }));
    let request = Request::builder()
        .method("POST")
        .uri("/requirements/req_shared/discussions")
        .header("host", "fixture.test")
        .header("authorization", "Bearer fixture-secret")
        .header("content-type", "application/json")
        .body(body)
        .unwrap();
    let router = host.router();
    let write = router.oneshot(request);
    tokio::pin!(write);
    assert!(poll!(&mut write).is_pending());
    tokio::time::advance(Duration::from_millis(2900)).await;
    release.send(()).unwrap();
    let response = write.await.unwrap();
    assert_eq!(response.status(), 200);
    host.shutdown().await;
}
