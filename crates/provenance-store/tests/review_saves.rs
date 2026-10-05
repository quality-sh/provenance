#[allow(dead_code)]
mod review_support;
use provenance_macros::verifies;
use review_support::*;
use serde_json::json;

#[test]
#[verifies("rule_review_revision_follows_review_content", examples)]
fn equal_review_content_has_equal_revision() {
    let (_temp, store) = fixture();
    let a = store
        .save_requirement(save(&store, json!({})))
        .unwrap();
    assert!(a.revision.is_some());
    let b = store
        .save_requirement(save(&store, json!({"description":"B"})))
        .unwrap();
    let again = store
        .save_requirement(save(
            &store,
            json!({"clear_fields":["description"]}),
        ))
        .unwrap();
    assert_ne!(a.revision, b.revision);
    assert_eq!(a.revision, again.revision);
    let lifecycle = store
        .save_requirement(save(&store, json!({"status":"active"})))
        .unwrap();
    assert_ne!(lifecycle.etag, again.etag);
    assert_eq!(lifecycle.revision, again.revision);
}

#[test]
fn stale_etag_and_wrong_owner_refuse_without_changing_state() {
    let (_temp, store) = fixture();
    store
        .save_requirement(save(&store, json!({})))
        .unwrap();
    let stale = save(&store, json!({"description":"stale"}));
    store
        .save_requirement(save(&store, json!({"status":"active"})))
        .unwrap();
    assert!(store.save_requirement(stale).is_err());
    let before = store.list_requirements(&scope()).unwrap();
    assert!(store
        .save_requirement(save(
            &store,
            json!({"declared_by":"other","description":"bad"})
        ))
        .is_err());
    assert_eq!(before, store.list_requirements(&scope()).unwrap());
    let etag = store.requirement_edit_state(&scope(), &id()).unwrap().etag;
    let bypassed = store
        .update_requirement(
            serde_json::from_value(
                json!({"scope_id":"default","id":"req_a","description":"bypass"}),
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(bypassed.description.as_deref(), Some("bypass"));
    assert_ne!(
        store.requirement_edit_state(&scope(), &id()).unwrap().etag,
        etag
    );
}

#[test]
fn concurrent_edits_with_one_etag_commit_exactly_once() {
    let (_temp, store) = fixture();
    store
        .save_requirement(save(&store, json!({})))
        .unwrap();
    let a = save(&store, json!({"description":"A"}));
    let b = save(&store, json!({"description":"B"}));
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
        .save_requirement(save(&store, json!({})))
        .unwrap();
    assert_eq!(std::fs::read(path).unwrap(), legacy);
}
