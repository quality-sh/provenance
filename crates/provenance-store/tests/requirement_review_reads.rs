use provenance_core::{ScopeId, SUPPORTED_SCHEMA_VERSION};
use provenance_store::layout::ProvenanceLayout;
use provenance_store::publication::publication_guard;
use provenance_store::state_store::StateStore;
use serde_json::json;
use std::sync::mpsc;
use std::time::Duration;

fn fixture() -> (tempfile::TempDir, ProvenanceLayout, StateStore, ScopeId) {
    let temp = tempfile::tempdir().unwrap();
    let root = camino::Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).unwrap();
    let layout = ProvenanceLayout::new(root);
    let store = StateStore::new(layout.clone());
    let scope = ScopeId::new("default").unwrap();
    (temp, layout, store, scope)
}

#[test]
fn requirement_review_read_refuses_a_future_layout() {
    let (_temp, layout, store, scope) = fixture();
    let path = layout
        .state_dir()
        .join("scopes/default/requirements/review.jsonl");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let future = SUPPORTED_SCHEMA_VERSION.0 + 1;
    let record = json!({
        "schema_version": future,
        "scope_id": "default",
        "id": "requirement_review_future",
        "rule_id": "rule_one",
        "requirement_id": "req_one",
        "field": "statement",
        "before": "The system reads records.",
        "after": "The system writes records.",
        "changed_at": 1
    });
    std::fs::write(&path, format!("{record}\n")).unwrap();

    let message = store
        .list_requirement_reviews(&scope)
        .unwrap_err()
        .to_string();

    assert!(message.contains("review.jsonl line 1"), "{message}");
    assert!(
        message.contains("record requirement_review_future"),
        "{message}"
    );
    assert!(
        message.contains(&format!("schema_version {future}")),
        "{message}"
    );
    assert!(
        message.contains(&format!(
            "schema_version {} only",
            SUPPORTED_SCHEMA_VERSION.0
        )),
        "{message}"
    );
}

#[tokio::test]
async fn requirement_review_read_waits_for_the_repository_guard() {
    let (_temp, layout, store, scope) = fixture();
    let guard = publication_guard(&layout).await.unwrap();
    let (started, ready) = mpsc::channel();
    let (sender, receiver) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        started.send(()).unwrap();
        sender.send(store.list_requirement_reviews(&scope)).unwrap();
    });
    ready.recv_timeout(Duration::from_secs(5)).unwrap();

    let before_release = receiver.recv_timeout(Duration::from_millis(300));
    drop(guard);

    assert!(matches!(
        before_release,
        Err(mpsc::RecvTimeoutError::Timeout)
    ));
    assert!(receiver
        .recv_timeout(Duration::from_secs(5))
        .unwrap()
        .unwrap()
        .is_empty());
    reader.join().unwrap();
}
