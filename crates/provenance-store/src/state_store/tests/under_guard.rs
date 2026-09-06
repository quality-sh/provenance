use super::seeded_source_requirement_store;
use crate::publication::{publication_guard, PublicationGuard};
use crate::state_store::StateStore;
use crate::test_probes::publication_lock_is_held;
use provenance_core::ScopeId;
use provenance_macros::verifies;
use std::sync::mpsc;
use std::time::Duration;

fn read_families(store: &StateStore, scope: &ScopeId) -> anyhow::Result<Vec<serde_json::Value>> {
    let mut records = Vec::new();
    macro_rules! read {
        ($($method:ident),+ $(,)?) => {
            $(records.push(serde_json::to_value(store.$method(scope)?)?);)+
        };
    }
    records.push(serde_json::to_value(store.manifest()?)?);
    records.push(serde_json::to_value(store.list_scope_directories()?)?);
    read!(
        list_sources,
        list_requirements,
        list_domains,
        list_boundaries,
        list_topics,
        list_questions,
        list_resolutions,
        list_rules,
        list_verification_bindings,
        list_implementation_bindings,
        list_threads,
        list_messages,
        list_contributions,
        list_synthesis_packets,
        list_proposal_cards,
        list_assertion_records,
        list_dispositions,
        list_ideation_landings,
        closed_sources,
        closed_requirements,
        closed_domains,
        closed_boundaries,
        closed_topics,
        closed_questions,
        closed_resolutions,
        closed_rules,
        closed_verification_bindings,
        closed_implementation_bindings,
    );
    records.push(serde_json::to_value(store.closed_manifest_scope(scope)?)?);
    store.validate_ideation_scope(scope)?;
    store.validate_graph_scope(scope)?;
    Ok(records)
}

#[tokio::test]
#[verifies("rule_store_under_guard_takes_no_second_lock", examples)]
async fn a_store_under_the_guard_takes_no_second_lock() {
    let root = camino::Utf8Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let snapshot =
        crate::publication::snapshot_state(&crate::layout::ProvenanceLayout::new(root)).unwrap();
    let store = StateStore::new(snapshot.layout().clone());
    let scope = ScopeId::new("default").unwrap();
    let expected = read_families(&store, &scope).unwrap();
    let guard = publication_guard(&store.layout).await.unwrap();
    let (sender, receiver) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        let guarded = StateStore::under_guard(&guard, store.layout.clone());
        let records = read_families(&guarded, &scope);
        sender
            .send((records, publication_lock_is_held(&store.layout)))
            .unwrap();
    });
    let (records, held) = receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("all families must return while the publication guard is held");
    assert_eq!(records.unwrap(), expected);
    assert!(
        held,
        "the publication guard must remain held through the reads"
    );
    reader.join().unwrap();
}

fn assert_waits_for_guard(store: StateStore, scope: ScopeId, guard: PublicationGuard) {
    let (started, ready) = mpsc::channel();
    let (sender, receiver) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        started.send(()).unwrap();
        sender.send(store.list_sources(&scope)).unwrap();
    });
    ready.recv_timeout(Duration::from_secs(5)).unwrap();
    let before_release = receiver.recv_timeout(Duration::from_millis(300));
    drop(guard);
    assert!(
        matches!(before_release, Err(mpsc::RecvTimeoutError::Timeout)),
        "a plain store must wait for the publication guard"
    );
    let sources = receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("the read must finish after the publication guard drops")
        .unwrap();
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].id.as_str(), "source_schads");
    reader.join().unwrap();
}

#[tokio::test]
#[verifies("rule_store_under_guard_takes_no_second_lock", examples)]
async fn a_plain_store_still_waits_for_the_lock() {
    let (_dir, store, scope) = seeded_source_requirement_store();
    let guard = publication_guard(&store.layout).await.unwrap();
    assert!(publication_lock_is_held(&store.layout));
    assert_waits_for_guard(store, scope, guard);
}

#[tokio::test]
#[verifies("rule_store_under_guard_takes_no_second_lock", examples)]
async fn a_cloned_store_locks_again() {
    let (_dir, store, scope) = seeded_source_requirement_store();
    let guard = publication_guard(&store.layout).await.unwrap();
    let cloned = StateStore::under_guard(&guard, store.layout.clone()).clone();
    assert_waits_for_guard(cloned, scope, guard);
}
