use crate::StatementHost;
use axum::{body::Body, http::Request};
use futures::{poll, stream};
use provenance_macros::verifies;
use serde_json::{json, Value};
use std::{convert::Infallible, time::Duration};
use tokio::sync::oneshot;
use tower::ServiceExt as _;

#[verifies("rule_host_slot_queue_bound", examples)]
#[verifies("rule_host_slot_refusal_kind", examples)]
#[tokio::test(start_paused = true)]
async fn http_ingress_and_execution_share_one_deadline() {
    let host = StatementHost::default();
    let mut workers = Vec::new();
    let mut work_releases = Vec::new();
    for _ in 0..8 {
        let execution = host.execution.clone();
        let (started, ready) = oneshot::channel();
        let (release, wait) = std::sync::mpsc::channel::<()>();
        workers.push(tokio::spawn(async move {
            execution
                .run("check-statement", move || {
                    let _ = started.send(());
                    let _ = wait.recv();
                    Ok(Value::Null)
                })
                .await
        }));
        ready.await.unwrap();
        work_releases.push(release);
    }
    let mut bodies = Vec::new();
    let mut body_releases = Vec::new();
    for _ in 0..8 {
        let (release, wait) = oneshot::channel();
        let body = Body::from_stream(stream::once(async move {
            let _ = wait.await;
            Ok::<_, Infallible>("?")
        }));
        let mut call = Box::pin(
            host.router().oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/statement-checks")
                    .body(body)
                    .unwrap(),
            ),
        );
        assert!(poll!(&mut call).is_pending());
        bodies.push(call);
        body_releases.push(release);
    }
    let (body_read, read) = oneshot::channel();
    let body = Body::from_stream(stream::once(async move {
        let _ = body_read.send(());
        Ok::<_, Infallible>(json!({"data":{"statement":"Stop."}}).to_string())
    }));
    let waiting = host.router().oneshot(
        Request::builder()
            .method("POST")
            .uri("/statement-checks")
            .body(body)
            .unwrap(),
    );
    tokio::pin!(waiting);
    let started = tokio::time::Instant::now();
    assert!(poll!(&mut waiting).is_pending());
    tokio::time::advance(Duration::from_secs(2)).await;
    body_releases.remove(0).send(()).unwrap();
    assert_eq!(bodies.remove(0).await.unwrap().status(), 400);
    assert!(poll!(&mut waiting).is_pending());
    read.await
        .expect("The request must pass ingress before the deadline");
    tokio::time::advance(Duration::from_millis(999)).await;
    assert!(poll!(&mut waiting).is_pending());
    tokio::time::advance(Duration::from_millis(1)).await;
    let response = tokio::time::timeout(Duration::ZERO, waiting).await;
    let elapsed = started.elapsed();
    // Release blocking work before assertions so a regression cannot hold the runner.
    drop(work_releases);
    for worker in workers {
        worker.await.unwrap().unwrap();
    }
    drop(bodies);
    drop(body_releases);
    host.shutdown().await;
    let response = response
        .expect("The slot wait must end at the request deadline")
        .unwrap();
    assert_eq!(elapsed, Duration::from_secs(3));
    assert_eq!(response.status(), 503);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body["error"]["kind"], "host_busy");
}
