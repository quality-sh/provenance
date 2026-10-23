#![cfg(feature = "test-fixture")]
//! Held HTTP bodies make ingress pressure deterministic through the public router.

mod support {
    pub mod records;
    pub mod resource_http;
}

use axum::{body::Body, http::Request};
use futures::{future::join_all, poll, stream};
use provenance_macros::verifies;
use provenance_transport::StatementHost;
use serde_json::json;
use std::{
    convert::Infallible,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    task::Poll,
    time::Duration,
};
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
    observed_read(host, || {})
}

fn observed_read(
    host: &StatementHost,
    admitted: impl FnOnce() + Send + 'static,
) -> (
    oneshot::Sender<()>,
    impl Future<Output = axum::response::Response>,
) {
    let (release, wait) = oneshot::channel();
    let body = Body::from_stream(stream::once(async move {
        admitted();
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

fn held_reads(
    host: &StatementHost,
) -> (
    Vec<oneshot::Sender<()>>,
    Vec<impl Future<Output = axum::response::Response>>,
) {
    (0..8).map(|_| held_read(host)).unzip()
}

type HeldRead = Pin<Box<dyn Future<Output = axum::response::Response> + Send>>;

fn full_ingress(host: &StatementHost) -> (Vec<oneshot::Sender<()>>, Vec<HeldRead>) {
    let (releases, reads) = held_reads(host);
    let mut reads: Vec<_> = reads
        .into_iter()
        .map(|read| Box::pin(read) as HeldRead)
        .collect();
    for read in &mut reads {
        assert!(poll!(read).is_pending());
    }
    (releases, reads)
}

#[verifies("rule_host_slot_queue_bound", examples)]
#[tokio::test]
async fn sixteen_reads_and_a_write_wait_for_ingress_slots() {
    let repo = Repository::new("The shared graph is readable.");
    repo.all_kinds();
    let host = host(&repo, true);
    let (releases, reads): (Vec<_>, Vec<_>) = held_reads(&host);
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
    let (later_releases, later_reads): (Vec<_>, Vec<_>) = held_reads(&host);
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

#[verifies("rule_host_slot_refusal_kind", examples)]
#[tokio::test(start_paused = true)]
async fn ingress_wait_has_a_bound_and_reports_host_busy() {
    let repo = Repository::new("The shared graph is readable.");
    repo.all_kinds();
    let host = support::resource_http::host(&repo, true);
    let (releases, reads): (Vec<_>, Vec<_>) = held_reads(&host);
    let reads = join_all(reads);
    tokio::pin!(reads);
    assert!(poll!(&mut reads).is_pending());
    let started = tokio::time::Instant::now();
    let waiting = call(&host, "GET", "/requirements/req_shared", None);
    tokio::pin!(waiting);
    assert!(poll!(&mut waiting).is_pending());
    tokio::time::advance(Duration::from_millis(2999)).await;
    assert!(poll!(&mut waiting).is_pending());
    tokio::time::advance(Duration::from_millis(1)).await;
    let (status, body) = waiting.await;
    assert_eq!(started.elapsed(), Duration::from_secs(3));
    assert_eq!(status, 503);
    assert_eq!(body["error"]["kind"], "host_busy");
    drop(releases);
    let _ = reads.await;
    host.shutdown().await;
}

#[verifies("rule_host_slot_refusal_kind", examples)]
#[tokio::test]
async fn shutdown_ends_ingress_waits_at_once() {
    let repo = Repository::new("The shared graph is readable.");
    repo.all_kinds();
    let host = host(&repo, true);
    let (_releases, reads): (Vec<_>, Vec<_>) = held_reads(&host);
    let reads = join_all(reads);
    tokio::pin!(reads);
    assert!(poll!(&mut reads).is_pending());
    let waiting = call(&host, "GET", "/requirements/req_shared", None);
    tokio::pin!(waiting);
    assert!(matches!(poll!(&mut waiting), Poll::Pending));
    host.shutdown().await;
    let (status, body) = tokio::time::timeout(Duration::from_millis(200), waiting)
        .await
        .unwrap();
    assert_eq!(status, 503);
    assert_eq!(body["error"]["kind"], "unavailable_needs");
    for response in reads.await {
        assert_eq!(response.status(), 503);
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["error"]["kind"], "unavailable_needs");
    }
    let (status, body) = call(&host, "GET", "/requirements/req_shared", None).await;
    assert_eq!(status, 503);
    assert_eq!(body["error"]["kind"], "unavailable_needs");
}

#[verifies("rule_host_slot_queue_bound", examples)]
#[tokio::test]
async fn ingress_waiters_keep_arrival_order() {
    let repo = Repository::new("The shared graph is readable.");
    repo.all_kinds();
    let host = host(&repo, true);
    let (mut releases, mut reads) = full_ingress(&host);
    let order = Arc::new(Mutex::new(Vec::new()));
    let mut queued = Vec::new();
    for index in 0..4 {
        let order = order.clone();
        let (release, read) = observed_read(&host, move || order.lock().unwrap().push(index));
        let mut read = Box::pin(read);
        assert!(poll!(&mut read).is_pending());
        queued.push((release, read));
    }
    assert!(order.lock().unwrap().is_empty());
    releases.remove(0).send(()).unwrap();
    assert_eq!(reads.remove(0).await.status(), 200);
    for index in 0..4 {
        // Reverse polling makes the semaphore order observable at the body read.
        for (_, read) in queued.iter_mut().rev() {
            assert!(poll!(read).is_pending());
        }
        assert_eq!(*order.lock().unwrap(), (0..=index).collect::<Vec<_>>());
        let (release, read) = queued.remove(0);
        release.send(()).unwrap();
        assert_eq!(read.await.status(), 200);
    }
    drop(releases);
    for read in reads {
        assert_eq!(read.await.status(), 200);
    }
    host.shutdown().await;
}

#[verifies("rule_host_slot_queue_bound", examples)]
#[tokio::test]
async fn dropped_ingress_waiter_frees_its_place() {
    let repo = Repository::new("The shared graph is readable.");
    repo.all_kinds();
    let host = host(&repo, true);
    let (mut releases, mut reads) = full_ingress(&host);
    let mut disconnected = Box::pin(call(&host, "GET", "/requirements/req_shared", None));
    assert!(poll!(&mut disconnected).is_pending());
    let next = call(&host, "GET", "/requirements/req_shared", None);
    tokio::pin!(next);
    assert!(poll!(&mut next).is_pending());
    drop(disconnected);
    releases.remove(0).send(()).unwrap();
    assert_eq!(reads.remove(0).await.status(), 200);
    let (status, body) = next.await;
    assert_eq!(status, 200, "{body}");
    drop(releases);
    for read in reads {
        assert_eq!(read.await.status(), 200);
    }
    host.shutdown().await;
}
