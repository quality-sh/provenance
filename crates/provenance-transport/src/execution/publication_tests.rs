use super::*;
use crate::fixture::records::Repository;
use provenance_core::ScopeId;
use provenance_store::{
    operations::catalog::{self, Apply, PreparedContext, PreparedScope},
    state_store::StateStore,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::oneshot;

fn apply(context: PreparedContext, runtime: &tokio::runtime::Handle) {
    let input = serde_json::from_value(serde_json::json!({
        "schema_version": provenance_core::SUPPORTED_SCHEMA_VERSION.0,
        "spec": "execution",
        "declared_by": "spec://execution",
        "requirements": [{"key":"saved", "statement":"The request preserves the saved graph."}]
    }))
    .unwrap();
    runtime
        .block_on(catalog::invoke_typed::<Apply>(context, input))
        .unwrap();
}

fn context(repo: &Repository) -> PreparedContext {
    PreparedContext::for_scope(PreparedScope {
        root: repo.dir.path().to_str().unwrap().into(),
        scope: ScopeId::new("default").unwrap(),
        requested_target: "selected".into(),
    })
}

fn requirements(repo: &Repository) -> usize {
    StateStore::new(repo.layout.clone())
        .list_requirements(&ScopeId::new("default").unwrap())
        .unwrap()
        .len()
}

#[tokio::test]
async fn a_panicked_task_after_publication_has_an_uncertain_outcome() {
    let repo = Repository::new("The graph is readable.");
    let before = requirements(&repo);
    let context = context(&repo);
    let runtime = tokio::runtime::Handle::current();
    let execution = Execution::default();
    let error = execution
        .run("apply", move || {
            apply(context, &runtime);
            panic!("injected response loss after publication");
        })
        .await
        .unwrap_err();
    execution.shutdown().await;
    assert_eq!(requirements(&repo), before + 1, "the write took effect");
    assert_eq!(error.error, serde_json::json!({"kind":"uncertain_write"}));
    assert_eq!(error.operation.as_deref(), Some("apply"));
}

#[tokio::test]
async fn a_disconnected_caller_keeps_publication_owned_until_shutdown() {
    let repo = Repository::new("The graph is readable.");
    let before = requirements(&repo);
    let context = context(&repo);
    let runtime = tokio::runtime::Handle::current();
    let execution = Execution::default();
    let worker = execution.clone();
    let submissions = Arc::new(AtomicUsize::new(0));
    let attempts = submissions.clone();
    let (release, continue_work) = oneshot::channel();
    let (published, ready) = oneshot::channel();
    let (finished, done) = oneshot::channel();
    let caller = tokio::spawn(async move {
        worker
            .run("apply", move || {
                attempts.fetch_add(1, Ordering::SeqCst);
                apply(context, &runtime);
                published.send(()).unwrap();
                continue_work.blocking_recv().unwrap();
                finished.send(()).unwrap();
                Ok(Value::Null)
            })
            .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(10), ready)
        .await
        .expect("the write must finish")
        .unwrap();
    caller.abort();
    let closer = execution.clone();
    let closing = tokio::spawn(async move {
        closer.shutdown().await;
    });
    tokio::task::yield_now().await;
    assert!(!closing.is_finished());
    release.send(()).unwrap();
    closing.await.unwrap();
    done.await.unwrap();
    assert_eq!(requirements(&repo), before + 1);
    assert_eq!(submissions.load(Ordering::SeqCst), 1);
}
