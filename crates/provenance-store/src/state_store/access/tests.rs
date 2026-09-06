use super::super::tests::seeded_source_requirement_store;
use crate::publication::{publication_guard, PublicationGuard};
use crate::state_store::StateStore;
use crate::test_probes::publication_lock_is_held;
use provenance_core::ScopeId;
use provenance_macros::verifies;
use std::sync::mpsc;
use std::time::Duration;

macro_rules! read_families {
    ($store:expr, $scope:expr) => {{
        let store = $store;
        let scope = $scope;
        (|| -> anyhow::Result<Vec<serde_json::Value>> {
            let records = vec![
                serde_json::to_value(store.manifest()?)?,
                serde_json::to_value(store.list_scope_directories()?)?,
                serde_json::to_value(store.list_sources(scope)?)?,
                serde_json::to_value(store.list_requirements(scope)?)?,
                serde_json::to_value(store.list_domains(scope)?)?,
                serde_json::to_value(store.list_boundaries(scope)?)?,
                serde_json::to_value(store.list_topics(scope)?)?,
                serde_json::to_value(store.list_questions(scope)?)?,
                serde_json::to_value(store.list_resolutions(scope)?)?,
                serde_json::to_value(store.list_rules(scope)?)?,
                serde_json::to_value(store.list_verification_bindings(scope)?)?,
                serde_json::to_value(store.active_verification_bindings(scope)?)?,
                serde_json::to_value(store.list_implementation_bindings(scope)?)?,
                serde_json::to_value(store.active_implementation_bindings(scope)?)?,
                serde_json::to_value(store.list_threads(scope)?)?,
                serde_json::to_value(store.list_messages(scope)?)?,
                serde_json::to_value(store.list_contributions(scope)?)?,
                serde_json::to_value(store.list_synthesis_packets(scope)?)?,
                serde_json::to_value(store.list_proposal_cards(scope)?)?,
                serde_json::to_value(store.list_proposal_definitions(scope)?)?,
                serde_json::to_value(store.list_dispositions(scope)?)?,
                serde_json::to_value(store.list_assertion_records(scope)?)?,
                serde_json::to_value(store.list_proposal_cards_with_actor_ids(
                    scope,
                    &store.manifest()?.disposition_actor_ids,
                )?)?,
            ];
            store.validate_ideation_scope(scope)?;
            store.validate_ideation_scope_with_actor_ids(
                scope,
                &store.manifest()?.disposition_actor_ids,
            )?;
            store.validate_graph_scope(scope)?;
            Ok(records)
        })()
    }};
}

fn closed_families(store: &StateStore, scope: &ScopeId) -> anyhow::Result<Vec<serde_json::Value>> {
    Ok(vec![
        serde_json::to_value(store.closed_sources(scope)?)?,
        serde_json::to_value(store.closed_requirements(scope)?)?,
        serde_json::to_value(store.closed_domains(scope)?)?,
        serde_json::to_value(store.closed_boundaries(scope)?)?,
        serde_json::to_value(store.closed_topics(scope)?)?,
        serde_json::to_value(store.closed_questions(scope)?)?,
        serde_json::to_value(store.closed_resolutions(scope)?)?,
        serde_json::to_value(store.closed_rules(scope)?)?,
        serde_json::to_value(store.closed_verification_bindings(scope)?)?,
        serde_json::to_value(store.closed_implementation_bindings(scope)?)?,
        serde_json::to_value(store.list_ideation_landings(scope)?)?,
        serde_json::to_value(store.closed_manifest_scope(scope)?)?,
    ])
}

#[tokio::test]
#[verifies("rule_store_under_guard_takes_no_second_lock", examples)]
#[verifies("rule_guarded_reads_use_guard_repository", examples)]
async fn a_store_under_the_guard_takes_no_second_lock() {
    let (_dir, store, scope) = seeded_source_requirement_store();
    let mut expected = read_families!(&store, &scope).unwrap();
    expected.extend(closed_families(&store, &scope).unwrap());
    let guard = publication_guard(&store.layout).await.unwrap();
    let (sender, receiver) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        let guarded = StateStore::under_guard(&guard);
        let records = read_families!(&guarded, &scope).and_then(|mut records| {
            records.extend(closed_families(&guarded.store, &scope)?);
            Ok(records)
        });
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

#[test]
fn a_plain_write_holds_the_publication_lock() {
    let (_dir, store, _scope) = seeded_source_requirement_store();
    let path = store.layout.state_dir().join("write_probe.jsonl");
    store
        .mutate_jsonl_records(&path, |records: &mut Vec<String>| {
            assert!(publication_lock_is_held(&store.layout));
            records.push("written".into());
            Ok(())
        })
        .unwrap();
    assert_eq!(std::fs::read_to_string(path).unwrap(), "\"written\"\n");
}

#[tokio::test]
async fn a_plain_write_waits_for_the_guard() {
    let (_dir, store, scope) = seeded_source_requirement_store();
    let guard = publication_guard(&store.layout).await.unwrap();
    assert!(publication_lock_is_held(&store.layout));
    let (started, ready) = mpsc::channel();
    let (sender, receiver) = mpsc::channel();
    let writer = std::thread::spawn(move || {
        started.send(()).unwrap();
        let result = store.set_requirement_fog(
            &scope,
            &provenance_core::StableId::new("req_overtime").unwrap(),
            Some("Changed".into()),
        );
        sender.send(result).unwrap();
    });
    ready.recv_timeout(Duration::from_secs(5)).unwrap();
    let before_release = receiver.recv_timeout(Duration::from_millis(300));
    drop(guard);
    assert!(matches!(
        before_release,
        Err(mpsc::RecvTimeoutError::Timeout)
    ));
    let requirement = receiver
        .recv_timeout(Duration::from_secs(5))
        .unwrap()
        .unwrap();
    assert_eq!(requirement.fog.as_deref(), Some("Changed"));
    writer.join().unwrap();
}
