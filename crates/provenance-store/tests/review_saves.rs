mod review_support;
use provenance_core::review::SaveOutcome;
use review_support::*;
use serde_json::json;

#[test]
fn repeated_content_has_distinct_occurrences_and_lifecycle_keeps_revision() {
    let (_temp, store) = fixture();
    let a = store
        .save_requirement(save(&store, "enroll", json!({})))
        .unwrap();
    assert_eq!(a.outcome, SaveOutcome::Enrolled);
    let b = store
        .save_requirement(save(&store, "edit_b", json!({"description":"B"})))
        .unwrap();
    let again = store
        .save_requirement(save(
            &store,
            "edit_a",
            json!({"clear_fields":["description"]}),
        ))
        .unwrap();
    assert_ne!(a.revision, b.revision);
    assert_ne!(a.revision, again.revision);
    assert_eq!(again.prior_revision, Some(b.revision));
    assert_eq!(a.after.digest, again.after.digest);
    let lifecycle = store
        .save_requirement(save(&store, "lifecycle", json!({"status":"active"})))
        .unwrap();
    assert_eq!(lifecycle.revision, again.revision);
    assert_eq!(lifecycle.outcome, SaveOutcome::LifecycleOnly);
    assert_ne!(lifecycle.etag, again.etag);
}

#[test]
fn request_replay_precedes_stale_cas_and_different_intent_refuses() {
    let (_temp, store) = fixture();
    let input = save(&store, "first", json!({}));
    let serialized = serde_json::to_value(&input).unwrap();
    let first = store.save_requirement(input).unwrap();
    store
        .save_requirement(save(&store, "next", json!({"description":"B"})))
        .unwrap();
    assert_eq!(
        first,
        store
            .save_requirement(serde_json::from_value(serialized.clone()).unwrap())
            .unwrap()
    );
    let mut different = serialized;
    different["update"]["description"] = json!("different");
    assert!(store
        .save_requirement(serde_json::from_value(different).unwrap())
        .is_err());
    let no_op = store
        .save_requirement(save(&store, "noop", json!({})))
        .unwrap();
    assert_eq!(no_op.outcome, SaveOutcome::NoChange);
}

#[test]
fn stale_etag_and_wrong_owner_refuse_without_changing_state() {
    let (_temp, store) = fixture();
    store
        .save_requirement(save(&store, "enroll", json!({})))
        .unwrap();
    let stale = save(&store, "stale", json!({"description":"stale"}));
    store
        .save_requirement(save(&store, "new", json!({"status":"active"})))
        .unwrap();
    assert!(store.save_requirement(stale).is_err());
    let before = store.list_requirements(&scope()).unwrap();
    assert!(store
        .save_requirement(save(
            &store,
            "owner",
            json!({"declared_by":"other","description":"bad"})
        ))
        .is_err());
    assert_eq!(before, store.list_requirements(&scope()).unwrap());
    assert!(store
        .update_requirement(
            serde_json::from_value(
                json!({"scope_id":"default","id":"req_a","description":"bypass"})
            )
            .unwrap()
        )
        .is_err());
}

#[test]
fn concurrent_edits_with_one_etag_commit_exactly_once() {
    let (_temp, store) = fixture();
    store
        .save_requirement(save(&store, "enroll", json!({})))
        .unwrap();
    let a = save(&store, "a", json!({"description":"A"}));
    let b = save(&store, "b", json!({"description":"B"}));
    let barrier = std::sync::Barrier::new(2);
    std::thread::scope(|threads| {
        let one = threads.spawn(|| {
            barrier.wait();
            store.save_requirement(a)
        });
        let two = threads.spawn(|| {
            barrier.wait();
            store.save_requirement(b)
        });
        let results = [one.join().unwrap(), two.join().unwrap()];
        assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
        assert!(results
            .iter()
            .find_map(|r| r.as_ref().err())
            .unwrap()
            .to_string()
            .contains("etag"));
    });
}

#[test]
fn enrollment_preserves_frozen_legacy_proposal_bytes() {
    let (temp, store) = fixture();
    let layout = provenance_store::layout::ProvenanceLayout::new(
        camino::Utf8Path::from_path(temp.path()).unwrap(),
    );
    let path = provenance_store::shards::proposal_cards_path(&layout, &scope());
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let legacy =
        b"{\"schema_version\":2,\"id\":\"old_proposal\",\"promotion_state\":\"accepted\"}\n";
    std::fs::write(&path, legacy).unwrap();
    store
        .save_requirement(save(&store, "enroll", json!({})))
        .unwrap();
    assert_eq!(std::fs::read(path).unwrap(), legacy);
}
