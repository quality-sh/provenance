use super::SaveRequirement;
use crate::{
    layout::ProvenanceLayout,
    state_store::StateStore,
    test_probes,
    write_error::{WriteError, WriteFailure},
};
use camino::Utf8Path;
use provenance_core::{ScopeId, StableId};
use serde_json::json;
use std::{sync::mpsc, time::Duration};

fn scope() -> ScopeId {
    ScopeId::new("default").unwrap()
}

fn requirement() -> StableId {
    StableId::new("req_a").unwrap()
}

fn fixture() -> (tempfile::TempDir, StateStore) {
    let temp = tempfile::tempdir().unwrap();
    let layout = ProvenanceLayout::new(Utf8Path::from_path(temp.path()).unwrap());
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    std::fs::write(
        layout.manifest_path(),
        r#"{"schema_version":2,"scopes":[{"id":"default","path_prefix":"."}]}"#,
    )
    .unwrap();
    let store = StateStore::new(layout);
    for id in ["req_a", "req_b", "req_c", "req_d"] {
        store
            .create_requirement(
                serde_json::from_value(json!({
                    "scope_id":"default", "id":id,
                    "statement":"The system stores records.",
                    "status":"discovery", "depends_on":[], "supersedes":[]
                }))
                .unwrap(),
            )
            .unwrap();
    }
    store
        .save_requirement(save(&store, "baseline", json!(null)))
        .unwrap();
    (temp, store)
}

fn save(store: &StateStore, request: &str, relationships: serde_json::Value) -> SaveRequirement {
    serde_json::from_value(json!({
        "request_id":request,
        "actor":"ben",
        "expected_etag":store.requirement_edit_state(&scope(), &requirement()).unwrap().etag,
        "update":{"scope_id":"default","id":"req_a"},
        "relationships":relationships
    }))
    .unwrap()
}

fn depends_on(store: &StateStore) -> Vec<String> {
    store
        .list_requirements(&scope())
        .unwrap()
        .into_iter()
        .find(|record| record.id == requirement())
        .unwrap()
        .depends_on
        .into_iter()
        .map(|id| id.as_str().to_owned())
        .collect()
}

fn hold_locked_save(
    store: &StateStore,
    input: SaveRequirement,
    entered: mpsc::Sender<()>,
    release: mpsc::Receiver<()>,
) -> anyhow::Result<provenance_core::review::ReviewEntry> {
    test_probes::arm("requirement_save_locked", move || {
        entered.send(()).unwrap();
        release.recv().unwrap();
        Ok(())
    });
    let result = store.save_requirement(input);
    test_probes::disarm("requirement_save_locked");
    result
}

#[test]
fn concurrent_partial_deltas_compose_from_locked_state() {
    let (_temp, store) = fixture();
    let first = save(&store, "add_b", json!({"depends_on":{"add":["req_b"]}}));
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (started_tx, started_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    std::thread::scope(|threads| {
        let first = threads.spawn(|| hold_locked_save(&store, first, entered_tx, release_rx));
        entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        let second = threads.spawn(|| {
            started_tx.send(()).unwrap();
            let result = store.save_requirement(save(
                &store,
                "add_c",
                json!({"depends_on":{"add":["req_c"]}}),
            ));
            done_tx.send(()).unwrap();
            result
        });
        started_rx.recv().unwrap();
        assert!(done_rx.recv_timeout(Duration::from_millis(50)).is_err());
        release_tx.send(()).unwrap();
        first.join().unwrap().unwrap();
        second.join().unwrap().unwrap();
    });
    assert_eq!(depends_on(&store), ["req_b", "req_c"]);
}

#[test]
fn relationship_delta_expands_under_the_publication_lock() {
    let (temp, store) = fixture();
    let layout = ProvenanceLayout::new(Utf8Path::from_path(temp.path()).unwrap());
    let (observed_tx, observed_rx) = mpsc::channel();
    test_probes::arm("requirement_relationships_expanding", move || {
        observed_tx
            .send(test_probes::publication_lock_is_held(&layout))
            .unwrap();
        Ok(())
    });

    store
        .save_requirement(save(
            &store,
            "locked_expand",
            json!({"depends_on":{"add":["req_b"]}}),
        ))
        .unwrap();
    test_probes::disarm("requirement_relationships_expanding");

    assert!(observed_rx.recv_timeout(Duration::from_secs(2)).unwrap());
}

#[test]
fn concurrent_final_set_and_delta_return_a_typed_conflict_without_corruption() {
    let (_temp, store) = fixture();
    let final_set = save(&store, "set_b", json!({"depends_on":["req_b"]}));
    let stale_delta = save(&store, "add_d", json!({"depends_on":{"add":["req_d"]}}));
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    std::thread::scope(|threads| {
        let first = threads.spawn(|| hold_locked_save(&store, final_set, entered_tx, release_rx));
        entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        let second = threads.spawn(|| store.save_requirement(stale_delta));
        release_tx.send(()).unwrap();
        first.join().unwrap().unwrap();
        let error = second.join().unwrap().unwrap_err();
        assert!(matches!(
            WriteError(error).safe(),
            WriteFailure::InvalidUpdate
        ));
    });
    assert_eq!(depends_on(&store), ["req_b"]);
}
