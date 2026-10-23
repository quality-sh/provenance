#![cfg(feature = "test-fixture")]
//! Implementation-aid tests: the coverage scan has no Rule for host queue limits.
//! Held HTTP bodies make ingress pressure deterministic through the public router.

mod support {
    pub mod records;
    pub mod resource_http;
}

use axum::{body::Body, http::Request};
use futures::{future::join_all, poll, stream};
use provenance_transport::StatementHost;
use serde_json::json;
use std::{convert::Infallible, future::Future, task::Poll, time::Duration};
use support::{
    records::Repository,
    resource_http::{call, host},
};
use tokio::sync::oneshot;
use tower::ServiceExt as _;

fn held_read(
    host: &StatementHost,
) -> (
    oneshot::Sender<()>,
    impl Future<Output = axum::response::Response>,
) {
    let (release, wait) = oneshot::channel();
    let body = Body::from_stream(stream::once(async move {
        let _ = wait.await;
        Ok::<_, Infallible>("")
    }));
    let request = Request::builder()
        .uri("/requirements/req_shared")
        .header("host", "fixture.test")
        .header("authorization", "Bearer fixture-secret")
        .body(body)
        .unwrap();
    let router = host.router();
    (
        release,
        async move { router.oneshot(request).await.unwrap() },
    )
}

#[tokio::test]
async fn sixteen_reads_and_a_write_wait_for_ingress_slots() {
    let repo = Repository::new("The shared graph is readable.");
    repo.all_kinds();
    let host = host(&repo, true);
    let (releases, reads): (Vec<_>, Vec<_>) = (0..8).map(|_| held_read(&host)).unzip();
    let reads = join_all(reads);
    tokio::pin!(reads);
    assert!(poll!(&mut reads).is_pending());
    let write = call(
        &host,
        "POST",
        "/requirements/req_shared/discussions",
        Some(json!({"data": {
            "actor":"reviewer", "declared_by":null, "role":"user", "body":"Check the burst."
        }})),
    );
    tokio::pin!(write);
    assert!(poll!(&mut write).is_pending(), "the write must queue");
    let (later_releases, later_reads): (Vec<_>, Vec<_>) = (0..8).map(|_| held_read(&host)).unzip();
    let later_reads = join_all(later_reads);
    tokio::pin!(later_reads);
    assert!(poll!(&mut later_reads).is_pending());
    for release in releases.into_iter().chain(later_releases) {
        let _ = release.send(());
    }
    let (reads, later_reads, (status, body)) = tokio::join!(reads, later_reads, write);
    for response in reads.into_iter().chain(later_reads) {
        assert_eq!(response.status(), 200);
    }
    assert_eq!(status, 200, "{body}");
    host.shutdown().await;
}

#[tokio::test]
async fn ingress_wait_has_a_bound_and_reports_host_busy() {
    let repo = Repository::new("The shared graph is readable.");
    repo.all_kinds();
    let host = support::resource_http::host(&repo, true);
    let (releases, reads): (Vec<_>, Vec<_>) = (0..8).map(|_| held_read(&host)).unzip();
    let reads = join_all(reads);
    tokio::pin!(reads);
    assert!(poll!(&mut reads).is_pending());
    let started = tokio::time::Instant::now();
    let (status, body) = call(&host, "GET", "/requirements/req_shared", None).await;
    assert!(started.elapsed() >= Duration::from_secs(2));
    assert!(started.elapsed() < Duration::from_secs(5));
    assert_eq!(status, 503);
    assert_eq!(body["error"]["kind"], "host_busy");
    drop(releases);
    let _ = reads.await;
    host.shutdown().await;
}

#[tokio::test]
async fn shutdown_ends_ingress_waits_at_once() {
    let repo = Repository::new("The shared graph is readable.");
    repo.all_kinds();
    let host = host(&repo, true);
    let (_releases, reads): (Vec<_>, Vec<_>) = (0..8).map(|_| held_read(&host)).unzip();
    let reads = join_all(reads);
    tokio::pin!(reads);
    assert!(poll!(&mut reads).is_pending());
    let waiting = call(&host, "GET", "/requirements/req_shared", None);
    tokio::pin!(waiting);
    assert!(matches!(poll!(&mut waiting), Poll::Pending));
    host.shutdown().await;
    let (status, _) = tokio::time::timeout(Duration::from_millis(200), waiting)
        .await
        .unwrap();
    assert_eq!(status, 503);
    for response in reads.await {
        assert_eq!(response.status(), 503);
    }
}
