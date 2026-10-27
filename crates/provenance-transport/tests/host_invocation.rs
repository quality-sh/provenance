#![cfg(feature = "test-fixture")]
//! One host serves a fixture repository, an RMCP duplex session and the Axum router. Every
//! timing test first awaits the `entered` signal that the gate sends from inside the blocking
//! work, so the held operation owns a work slot before the test acts.

#[allow(dead_code)]
mod support {
    pub mod gate;
    pub mod mcp;
    pub mod records;
    pub mod resource_http;
}

use axum::{body::Body, http::Request};
use futures::poll;
use provenance_core::{
    protocol::{
        failure::{OperationError, OperationFailure},
        ResolveRecordQuery,
    },
    NodeType, SDK_PROTOCOL_VERSION,
};
use provenance_macros::verifies;
use provenance_store::{
    operations::catalog::ResolveRecord, publication::with_repository_publication,
};
use provenance_transport::{Invocation, StatementHost};
use serde_json::json;
use std::{sync::Arc, time::Duration};
use support::{
    gate::{Control, Gate},
    mcp::{call, Session},
    records::Repository,
    resource_http::access,
};
use tower::ServiceExt as _;

/// The publication lock, held on a thread outside the host until released.
struct HeldLock {
    release: std::sync::mpsc::Sender<()>,
    holder: std::thread::JoinHandle<()>,
}

impl HeldLock {
    fn hold(repo: &Repository) -> Self {
        let (held, wait_held) = std::sync::mpsc::channel();
        let (release, wait_release) = std::sync::mpsc::channel::<()>();
        let layout = repo.layout.clone();
        let holder = std::thread::spawn(move || {
            with_repository_publication(&layout, || {
                held.send(()).unwrap();
                let _ = wait_release.recv();
                Ok(())
            })
            .unwrap();
        });
        wait_held.recv().unwrap();
        Self { release, holder }
    }

    fn release(self) {
        self.release.send(()).unwrap();
        self.holder.join().unwrap();
    }
}

fn gated_host(repo: &Repository, open: bool) -> (StatementHost, Control) {
    let (gate, control) = Gate::new(access(repo).allow_writes(), open);
    (StatementHost::with_access(Arc::new(gate)), control)
}

fn discuss() -> serde_json::Value {
    json!({
        "parent":{"node_type":"requirement","node_id":"req_shared"},
        "actor":"reviewer", "role":"user", "body":"Check the write."
    })
}

async fn statement_check(host: &StatementHost) -> u16 {
    let request = Request::builder()
        .method("POST")
        .uri("/statement-checks")
        .header("host", "fixture.test")
        .header("authorization", "Bearer fixture-secret")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"data":{"statement":"Stop."}}).to_string(),
        ))
        .unwrap();
    host.router()
        .oneshot(request)
        .await
        .unwrap()
        .status()
        .as_u16()
}

/// A data-free call answers within one second, so no runtime thread is blocked.
async fn assert_statement_check_answers(host: &StatementHost) {
    let status = tokio::time::timeout(Duration::from_secs(1), statement_check(host))
        .await
        .expect("the data-free call answers while the other call waits");
    assert_eq!(status, 200);
}

fn discussion_exists(repo: &Repository, discussion_id: &str) -> bool {
    std::fs::read_to_string(
        repo.dir
            .path()
            .join(".provenance/state/scopes/default/threads/discussions.jsonl"),
    )
    .is_ok_and(|rows| rows.contains(discussion_id))
}

#[verifies("rule_lock_wait_does_not_delay_host_calls", examples)]
#[tokio::test]
async fn a_typed_write_waiting_for_the_lock_does_not_delay_a_data_free_call() {
    let repo = Repository::new("The shared graph is readable.");
    let (host, mut control) = gated_host(&repo, true);
    let lock = HeldLock::hold(&repo);
    let session = Session::start(host.clone()).await;
    let mut write = Box::pin(call(&session.client, "discuss", discuss()));
    assert!(poll!(&mut write).is_pending());
    control.entered().await;

    assert_statement_check_answers(&host).await;

    lock.release();
    let written = write.await;
    assert_ne!(written.is_error, Some(true), "{written:?}");
    let discussion_id = written.structured_content.as_ref().unwrap()["receipt"]["discussion_id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(discussion_exists(&repo, &discussion_id));
    session.close().await;
    host.shutdown().await;
}

#[verifies("rule_lock_wait_does_not_delay_host_calls", examples)]
#[tokio::test]
async fn a_slow_repository_preparation_does_not_delay_a_data_free_call() {
    let repo = Repository::new("The shared graph is readable.");
    let (host, mut control) = gated_host(&repo, false);
    let session = Session::start(host.clone()).await;
    let mut read = Box::pin(call(&session.client, "get", json!({"target":"req_shared"})));
    assert!(poll!(&mut read).is_pending());
    control.entered().await;

    assert_statement_check_answers(&host).await;

    control.open();
    let read = read.await;
    assert_ne!(read.is_error, Some(true), "{read:?}");
    assert_eq!(
        read.structured_content.as_ref().unwrap()["record"]["id"],
        "req_shared"
    );
    session.close().await;
    host.shutdown().await;
}

#[verifies("rule_host_shutdown_completes_started_work", examples)]
#[tokio::test]
async fn shutdown_waits_for_a_typed_write() {
    let repo = Repository::new("The shared graph is readable.");
    let (host, mut control) = gated_host(&repo, true);
    let lock = HeldLock::hold(&repo);
    let session = Session::start(host.clone()).await;
    let mut write = Box::pin(call(&session.client, "discuss", discuss()));
    assert!(poll!(&mut write).is_pending());
    control.entered().await;

    let closing = host.shutdown();
    tokio::pin!(closing);
    assert!(poll!(&mut closing).is_pending());
    tokio::task::yield_now().await;
    assert!(poll!(&mut closing).is_pending());

    lock.release();
    closing.await;
    let written = write.await;
    assert_ne!(written.is_error, Some(true), "{written:?}");
    let discussion_id = written.structured_content.as_ref().unwrap()["receipt"]["discussion_id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(discussion_exists(&repo, &discussion_id));
    session.close().await;
}

fn resolve_shared() -> ResolveRecordQuery {
    ResolveRecordQuery {
        protocol_version: Some(SDK_PROTOCOL_VERSION),
        id: "req_shared".into(),
        allowed_node_types: NodeType::ALL.to_vec(),
    }
}

#[verifies("rule_host_refusal_before_slot_wait", examples)]
#[tokio::test(start_paused = true)]
async fn a_refused_identity_does_not_wait_for_a_work_slot() {
    let repo = Repository::new("The shared graph is readable.");
    let (host, mut control) = gated_host(&repo, false);
    let mut held = Vec::new();
    for _ in 0..8 {
        let worker = host.clone();
        held.push(tokio::spawn(async move {
            worker
                .invoke::<ResolveRecord>(resolve_shared(), Invocation::native())
                .await
        }));
        control.entered().await;
    }

    control.refuse_identities();
    let refused = host.invoke::<ResolveRecord>(resolve_shared(), Invocation::native());
    tokio::pin!(refused);
    let answer = poll!(&mut refused);
    assert!(
        matches!(
            answer,
            std::task::Poll::Ready(Err(OperationError::Common(OperationFailure::AccessDenied)))
        ),
        "the refused call answers before any slot wait: {answer:?}"
    );

    control.open();
    for call in held {
        call.await.unwrap().unwrap();
    }
    host.shutdown().await;
}
