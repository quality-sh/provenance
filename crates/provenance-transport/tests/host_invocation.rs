#![cfg(feature = "test-fixture")]
//! One host serves a fixture repository and the Axum router. Every
//! timing test first awaits the `entered` signal that the gate sends from inside the blocking
//! work, so the held operation owns a work slot before the test acts.

#[allow(dead_code)]
mod support {
    pub mod gate;
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
use provenance_store::operations::catalog::{self, ResolveRecord};
use provenance_transport::{Invocation, StatementHost};
use serde_json::json;
use std::{sync::Arc, time::Duration};
use support::{
    gate::{Control, Gate},
    records::Repository,
    resource_http::access,
};
use tower::ServiceExt as _;

fn gated_host(repo: &Repository, open: bool) -> (StatementHost, Control) {
    let (gate, control) = Gate::new(access(repo).allow_writes(), open);
    let host = StatementHost::with_access(Arc::new(gate))
        .with_credential(Arc::new(access(repo).allow_writes()));
    (host, control)
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
    let held = fill_work_slots(&host, &mut control).await;

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

    release_held(&control, held).await;
    host.shutdown().await;
}

type Resolved = Result<
    <ResolveRecord as catalog::Operation>::Success,
    OperationError<<ResolveRecord as catalog::Operation>::Failure>,
>;

/// Eight typed calls held in the gate fill the work slots and take no ingress slot.
async fn fill_work_slots(
    host: &StatementHost,
    control: &mut Control,
) -> Vec<tokio::task::JoinHandle<Resolved>> {
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
    held
}

/// Resumes the clock, opens the gate and waits for the held calls. Each held call opens and
/// closes the repository cache, and a close that meets another close waits for the close
/// lock on a runtime timer. A paused clock does not advance while blocking work runs, so
/// that timer does not fire until the clock runs again.
async fn release_held(control: &Control, held: Vec<tokio::task::JoinHandle<Resolved>>) {
    tokio::time::resume();
    control.open();
    for call in held {
        call.await.unwrap().unwrap();
    }
}

/// A close lock held over the gate opening makes a held call's cache close wait on a runtime
/// timer, as two closes that meet do. The test holds the store's internal close lock file and
/// releases it only after a close has opened that file and failed to take the lock.
///
/// This test helps the paused-clock cleanup in these tests. It pins an internal failure
/// mechanism and defines no product obligation, so it has no Rule binding. It reads
/// `/proc/self/fd` to find the close, so it runs only on Linux.
#[cfg(target_os = "linux")]
#[tokio::test(start_paused = true)]
async fn held_calls_finish_when_their_cache_closes_meet() {
    let repo = Repository::new("The shared graph is readable.");
    let (host, mut control) = gated_host(&repo, false);
    let held = fill_work_slots(&host, &mut control).await;
    let path = repo.layout.cache_db_path().with_extension("db.close.lock");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let lock = std::fs::File::create(&path).unwrap();
    fs2::FileExt::lock_exclusive(&lock).unwrap();
    let path = path.canonicalize().unwrap();
    let (opened, close_opened) = tokio::sync::oneshot::channel();
    std::thread::spawn(move || {
        while !opened.is_closed() {
            if handles_to(&path) > 1 {
                let _ = opened.send(());
                return;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    });
    // A close opens the lock file and tries the lock in one poll on this runtime thread, so
    // when this task runs, the close that opened the file waits on its retry timer.
    let unlock = tokio::spawn(async move {
        close_opened.await.unwrap();
        drop(lock);
    });
    let (stall, stalled) = tokio::sync::oneshot::channel();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(10));
        let _ = stall.send(());
    });

    let release = release_held(&control, held);
    tokio::pin!(release);
    let stalled = tokio::select! {
        () = &mut release => false,
        _ = stalled => true,
    };
    if stalled {
        tokio::time::resume();
        release.await;
    }
    let close_waited = unlock.is_finished();
    unlock.abort();
    host.shutdown().await;
    assert!(!stalled, "the held calls stalled behind the paused clock");
    assert!(close_waited, "no cache close waited on the close lock");
}

/// Counts the open file descriptors of this process that refer to `path`.
#[cfg(target_os = "linux")]
fn handles_to(path: &std::path::Path) -> usize {
    std::fs::read_dir("/proc/self/fd")
        .unwrap()
        .filter_map(|entry| std::fs::read_link(entry.ok()?.path()).ok())
        .filter(|target| target == path)
        .count()
}

/// A check port for builder-order tests; the MCP `check` tool is not exercised here.
struct NoCheck;

impl provenance_porcelain::check::CheckPort for NoCheck {
    fn run<'a>(
        &'a self,
        _: provenance_porcelain::check::Category,
        _: Option<&'a str>,
    ) -> provenance_porcelain::check::PortFuture<'a> {
        Box::pin(async { Err("no check port".to_owned()) })
    }
}

async fn metadata(host: &StatementHost, bearer: bool) -> u16 {
    let mut request = Request::builder()
        .method("GET")
        .uri("/metadata")
        .header("host", "fixture.test");
    if bearer {
        request = request.header("authorization", "Bearer fixture-secret");
    }
    host.router()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap()
        .status()
        .as_u16()
}

async fn requirement_without_bearer(host: &StatementHost) -> (u16, serde_json::Value) {
    let request = Request::builder()
        .method("GET")
        .uri("/requirements/req_shared")
        .header("host", "fixture.test")
        .body(Body::empty())
        .unwrap();
    let response = host.router().oneshot(request).await.unwrap();
    let status = response.status().as_u16();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, serde_json::from_slice(&body).unwrap())
}

#[tokio::test]
async fn the_default_host_permits_data_free_calls_only() {
    let host = StatementHost::default();
    assert_eq!(statement_check(&host).await, 200);

    let (status, body) = requirement_without_bearer(&host).await;
    assert_eq!(status, 403, "{body}");
    assert_eq!(body["error"]["kind"], "access_denied");

    let refused = host
        .invoke::<ResolveRecord>(resolve_shared(), Invocation::native())
        .await;
    assert!(matches!(
        refused,
        Err(OperationError::Common(OperationFailure::AccessDenied))
    ));
    host.shutdown().await;
}

#[verifies("rule_bound_router_requires_credential_policy", examples)]
#[tokio::test]
async fn the_router_stays_closed_until_a_credential_policy_is_set_in_every_builder_order() {
    let repo = Repository::new("The shared graph is readable.");
    let binding = || -> Arc<dyn provenance_transport::HostAccess> { Arc::new(access(&repo)) };
    let credential =
        || -> Arc<dyn provenance_transport::HttpCredential> { Arc::new(access(&repo)) };
    let page_writes = |host: StatementHost| host.with_page_writes(repo.open(), Arc::new(|_| {}));
    let check_port = |host: StatementHost| host.with_check_port(Arc::new(NoCheck));

    let closed = [
        StatementHost::with_access(binding()),
        page_writes(StatementHost::with_access(binding())),
        check_port(StatementHost::with_access(binding())),
        check_port(page_writes(StatementHost::with_access(binding()))),
    ];
    for host in &closed {
        assert_eq!(metadata(host, true).await, 401);
        assert_eq!(metadata(host, false).await, 401);
    }

    let open = [
        StatementHost::with_access(binding()).with_credential(credential()),
        page_writes(StatementHost::with_access(binding()).with_credential(credential())),
        check_port(StatementHost::with_access(binding())).with_credential(credential()),
        page_writes(check_port(StatementHost::with_access(binding())))
            .with_credential(credential()),
        StatementHost::with_fixture_access(access(&repo)),
    ];
    for host in &open {
        assert_eq!(metadata(host, true).await, 200);
        assert_eq!(metadata(host, false).await, 401);
    }
}
