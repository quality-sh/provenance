use super::super::tests::seeded_source_requirement_store;
use crate::cache::tests::fixtures::open;
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

fn landings(store: &StateStore, scope: &ScopeId) -> anyhow::Result<Vec<serde_json::Value>> {
    Ok(vec![serde_json::to_value(
        store.list_ideation_landings(scope)?,
    )?])
}

#[tokio::test]
#[verifies("rule_store_under_guard_takes_no_second_lock", examples)]
#[verifies("rule_guarded_reads_use_guard_repository", examples)]
async fn a_store_under_the_guard_takes_no_second_lock() {
    let (_dir, store, scope) = seeded_source_requirement_store();
    let mut expected = read_families!(&store, &scope).unwrap();
    expected.extend(landings(&store, &scope).unwrap());
    let guard = publication_guard(&open(&store.layout)).await.unwrap();
    let (sender, receiver) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        let guarded = StateStore::under_guard(&guard);
        let records = read_families!(&guarded, &scope).and_then(|mut records| {
            records.extend(landings(&guarded.store, &scope)?);
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
    let guard = publication_guard(&open(&store.layout)).await.unwrap();
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
    let guard = publication_guard(&open(&store.layout)).await.unwrap();
    assert!(publication_lock_is_held(&store.layout));
    let (started, ready) = mpsc::channel();
    let (sender, receiver) = mpsc::channel();
    let writer = std::thread::spawn(move || {
        started.send(()).unwrap();
        let result = store.update_requirement(
            serde_json::from_value(serde_json::json!({
                "scope_id": scope.as_str(), "id": "req_overtime", "description": "Changed"
            }))
            .unwrap(),
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
    assert_eq!(requirement.description.as_deref(), Some("Changed"));
    writer.join().unwrap();
}

#[test]
#[verifies("rule_enrolled_record_write_needs_store_permission", examples)]
fn a_live_store_that_holds_the_section_gets_no_write_permission() {
    let (_dir, store, scope) = seeded_source_requirement_store();
    let repository = open(&store.layout);
    let path = crate::shards::requirements_path(&store.layout, &scope);
    let before = std::fs::read(&path).unwrap();
    let restate = |live: &StateStore| {
        live.mutate_jsonl_records(&path, |records: &mut Vec<provenance_core::Requirement>| {
            records[0].statement = "The system keeps records.".into();
            Ok(())
        })
    };
    let refused = |live: &StateStore| {
        live.permitting(&[&path], "req_overtime", restate)
            .unwrap_err()
            .to_string()
            .contains("needs a staged store")
    };
    crate::publication::with_publication_guard(&repository, |guard| {
        let guarded = guard.repository().store();
        assert!(refused(&guarded), "the store of the guard repository");
        guarded.with_repository_publication(|held| {
            assert!(refused(held), "the store that the section hands");
            Ok(())
        })?;
        assert!(
            restate(&guarded).is_err(),
            "no permission, no enrolled write"
        );
        Ok(())
    })
    .unwrap();
    assert!(refused(&repository.store()), "a store that takes the lock");
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

/// Implementation aid: a fenced store reads only its directories. A read
/// outside them fails the result, also when the reader drops the error.
/// No Rule binds it.
#[test]
fn a_fenced_store_refuses_a_read_outside_its_directories() {
    let (_dir, store, scope) = seeded_source_requirement_store();
    let repository = open(&store.layout);
    let view = std::sync::Arc::new(repository.storage().read_view().unwrap());
    let fenced = |directories| {
        super::ViewStore::new(&repository, std::sync::Arc::clone(&view)).fenced(&scope, directories)
    };
    let inside = fenced(vec!["requirements"])
        .read(|store| store.list_requirements(&scope))
        .unwrap();
    assert_eq!(inside.len(), 1);
    let error = fenced(vec!["requirements"])
        .read(|store| {
            let _dropped = store.list_sources(&scope);
            store.list_requirements(&scope)
        })
        .unwrap_err();
    assert!(
        error.to_string().contains("outside its input directories"),
        "{error:#}"
    );
}

/// The full reads and the hashes of the state tree that `run` makes on this
/// thread.
fn tree_reads_and_hashes<R>(run: impl FnOnce() -> R) -> (R, [usize; 2]) {
    let counts = std::rc::Rc::new(std::cell::Cell::new([0, 0]));
    for (label, index) in [("state_tree_read", 0), ("state_tree_hashed", 1)] {
        let counts = counts.clone();
        crate::test_probes::arm(label, move || {
            let mut now = counts.get();
            now[index] += 1;
            counts.set(now);
            Ok(())
        });
    }
    let result = run();
    crate::test_probes::disarm("state_tree_read");
    crate::test_probes::disarm("state_tree_hashed");
    (result, counts.get())
}

/// Implementation aid: a record save reads the tree for the section view and
/// for the base check, and hashes nothing. No Rule binds it.
#[test]
fn a_record_save_in_a_live_section_reads_the_tree_twice_and_hashes_nothing() {
    let (_dir, store, scope) = seeded_source_requirement_store();
    let repository = open(&store.layout);
    let counts = crate::publication::with_publication_guard(&repository, |guard| {
        let live = guard.repository().store();
        let (saved, counts) = tree_reads_and_hashes(|| {
            live.update_requirement(serde_json::from_value(serde_json::json!({
                "scope_id": scope.as_str(), "id": "req_overtime", "description": "Changed"
            }))?)
        });
        assert_eq!(saved?.description.as_deref(), Some("Changed"));
        Ok(counts)
    })
    .unwrap();
    assert_eq!(counts, [2, 0], "full reads and hashes of the state tree");
}

/// Implementation aid: a per-key write with no view in the section reads and
/// hashes nothing. No Rule binds it.
#[test]
fn a_per_key_write_before_any_read_in_the_section_reads_and_hashes_nothing() {
    let (_dir, store, _scope) = seeded_source_requirement_store();
    let repository = open(&store.layout);
    let path = store.layout.state_dir().join("write_probe.jsonl");
    let counts = crate::publication::with_publication_guard(&repository, |guard| {
        let live = guard.repository().store();
        let (written, counts) =
            tree_reads_and_hashes(|| live.publish_shard(&path, b"\"written\"\n".to_vec()));
        written?;
        Ok(counts)
    })
    .unwrap();
    assert_eq!(counts, [0, 0], "full reads and hashes of the state tree");
    assert_eq!(std::fs::read_to_string(path).unwrap(), "\"written\"\n");
}
