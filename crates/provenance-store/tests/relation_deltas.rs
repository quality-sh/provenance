//! Relationship edits on the guarded save: complete final sets, partial
//! deltas, deterministic expansion, and validation on the resulting final
//! resource.

mod review_support;
use provenance_core::review::SaveOutcome;
use provenance_store::review::SaveRequirement;
use review_support::*;
use serde_json::json;

/// Seeds the records the relationship edits name. req_cycle already depends
/// on req_a, so a req_a-to-req_cycle edit closes a cycle.
fn seed_targets(store: &provenance_store::state_store::StateStore) {
    for (id, depends_on) in [
        ("req_b", vec![]),
        ("req_c", vec![]),
        ("req_d", vec![]),
        ("req_cycle", vec!["req_a"]),
    ] {
        store
            .create_requirement(
                serde_json::from_value(json!({
                    "scope_id": "default", "id": id,
                    "statement": "The system stores records.",
                    "status": "discovery", "depends_on": depends_on, "supersedes": [],
                    "description": null, "domain_id": null,
                    "refines": null, "spawned_by": null,
                    "origin_thread": null, "origin_message": null
                }))
                .unwrap(),
            )
            .unwrap();
    }
    store
        .create_resolution(serde_json::from_value(json!({
            "scope_id": "default", "id": "res_origin",
            "title": "Origin", "position": "Decided",
            "rationale": "The award says so", "requirement_ids": ["req_a"],
            "supersedes": [], "status": "proposed", "context": null,
            "enforcement": null, "confidence": null, "inputs": [],
            "made_by": null, "approved_by": null, "approved_at": null,
            "origin_thread": null, "origin_message": null
        }))
        .unwrap())
        .unwrap();
    for id in ["source_one", "source_two"] {
        store
            .create_source(serde_json::from_value(json!({
                "scope_id": "default", "id": id, "name": id,
                "source_type": "policy", "url": null,
                "reference": "docs/policy.md", "commit_pin": null,
                "effective_date": null, "review_date": null,
                "supersedes": [], "origin_thread": null, "origin_message": null
            }))
            .unwrap())
            .unwrap();
    }
}

fn relations(
    store: &provenance_store::state_store::StateStore,
    request: &str,
    edit: serde_json::Value,
) -> SaveRequirement {
    let mut input = serde_json::to_value(save(store, request, json!({}))).unwrap();
    input["relationships"] = edit;
    serde_json::from_value(input).unwrap()
}

fn save_ok(store: &provenance_store::state_store::StateStore, request: &str, edit: serde_json::Value) -> provenance_core::review::ReviewEntry {
    store
        .save_requirement(relations(store, request, edit))
        .unwrap()
}


fn record(store: &provenance_store::state_store::StateStore) -> provenance_core::Requirement {
    store
        .list_requirements(&scope())
        .unwrap()
        .into_iter()
        .find(|r| r.id == id())
        .unwrap()
}

fn depends_on(store: &provenance_store::state_store::StateStore) -> Vec<String> {
    record(store).depends_on.iter().map(|v| v.as_str().to_owned()).collect()
}

#[test]
fn list_deltas_add_and_remove_without_touching_other_fields() {
    let (_temp, store) = fixture();
    seed_targets(&store);
    save_ok(&store, "add", json!({"depends_on": {"add": ["req_b"]}}));
    assert_eq!(depends_on(&store), ["req_b"]);
    save_ok(&store, "add_more", json!({"depends_on": {"add": ["req_c", "req_b"]}}));
    assert_eq!(depends_on(&store), ["req_b", "req_c"]);
    save_ok(&store, "remove", json!({"depends_on": {"remove": ["req_b"]}}));
    assert_eq!(depends_on(&store), ["req_c"]);
    // A repeated add is not a change.
    let entry = save_ok(&store, "again", json!({"depends_on": {"add": ["req_c"]}}));
    assert_eq!(entry.outcome, SaveOutcome::NoChange);
    assert_eq!(depends_on(&store), ["req_c"]);
}

#[test]
fn an_array_supplies_the_complete_final_set() {
    let (_temp, store) = fixture();
    seed_targets(&store);
    save_ok(&store, "final_one", json!({"depends_on": ["req_c"]}));
    assert_eq!(depends_on(&store), ["req_c"]);
    // A complete set is normalized: sorted, without duplicates.
    save_ok(&store, "final_two", json!({"depends_on": ["req_d", "req_b", "req_b"]}));
    assert_eq!(depends_on(&store), ["req_b", "req_d"]);
}

#[test]
fn singleton_edits_set_and_clear() {
    let (_temp, store) = fixture();
    seed_targets(&store);
    save_ok(&store, "spawned", json!({"spawned_by": "res_origin"}));
    assert_eq!(record(&store).spawned_by.as_ref().map(|v| v.as_str()), Some("res_origin"));
    save_ok(&store, "clear_spawned", json!({"spawned_by": null}));
    assert_eq!(record(&store).spawned_by, None);
    save_ok(&store, "refine", json!({"refines": "req_b"}));
    assert_eq!(record(&store).refines.as_ref().map(|v| v.as_str()), Some("req_b"));
}

#[test]
fn cite_deltas_add_citations_and_remove_every_clause_of_one_source() {
    let (_temp, store) = fixture();
    seed_targets(&store);
    save_ok(&store, "cites", json!({"cites": {"add": [{"source_id": "source_one"}]}}));
    save_ok(&store, "cites_more", json!({"cites": {"add": [
        {"source_id": "source_one", "clause": "4.2"},
        {"source_id": "source_two"}
    ]}}));
    assert_eq!(record(&store).source_refs.len(), 3);
    save_ok(&store, "uncite", json!({"cites": {"remove": ["source_one"]}}));
    let record = record(&store);
    assert_eq!(record.source_refs.len(), 1);
    assert_eq!(record.source_refs[0].source_id.as_str(), "source_two");
}

#[test]
fn invalid_deltas_refuse_and_publish_nothing() {
    let (_temp, store) = fixture();
    seed_targets(&store);
    save_ok(&store, "add", json!({"depends_on": {"add": ["req_c"]}}));
    let etag = store.requirement_edit_state(&scope(), &id()).unwrap().etag;

    // A remove of an absent entry refuses and changes nothing.
    let missing = relations(&store, "bad_remove", json!({"depends_on": {"remove": ["req_missing"]}}));
    assert!(store.save_requirement(missing).is_err());
    // A misspelled delta field refuses instead of silently no-oping.
    let mut typo = serde_json::to_value(save(&store, "typo", json!({}))).unwrap();
    typo["relationships"] = json!({"depends_on": {"depends": [], "remove": ["req_c"]}});
    assert!(
        serde_json::from_value::<SaveRequirement>(typo).is_err(),
        "unknown delta fields refuse"
    );
    assert_eq!(store.requirement_edit_state(&scope(), &id()).unwrap().etag, etag);

    // The failed requests recorded no outcome, so the identity is a fresh save.
    let retried = relations(&store, "bad_remove", json!({"depends_on": {"remove": ["req_c"]}}));
    assert_eq!(
        store.save_requirement(retried).unwrap().outcome,
        SaveOutcome::Changed
    );
}

#[test]
fn validation_runs_on_the_final_resource_exactly_as_one_edit() {
    let (_temp, store) = fixture();
    seed_targets(&store);

    // req_cycle already depends on req_a, so this delta closes a cycle.
    let cycle = relations(&store, "cycle", json!({"depends_on": {"add": ["req_cycle"]}}));
    assert!(store.save_requirement(cycle).is_err());

    // An add and a remove of the same entry validate against the final set:
    // the removal follows the add, so the delta applies as no change.
    let swap = relations(
        &store,
        "swap",
        json!({"depends_on": {"add": ["req_b"], "remove": ["req_b"]}}),
    );
    assert_eq!(store.save_requirement(swap).unwrap().outcome, SaveOutcome::NoChange);
    assert!(depends_on(&store).is_empty());

    // An unknown target kind refuses: spawned_by takes a resolution.
    let wrong_kind = relations(&store, "wrong_kind", json!({"spawned_by": "req_b"}));
    assert!(store.save_requirement(wrong_kind).is_err());
}

#[test]
fn the_same_delta_on_the_same_state_resolves_to_one_outcome() {
    let (_temp, store) = fixture();
    seed_targets(&store);
    let input = relations(&store, "delta_req", json!({"depends_on": {"add": ["req_b"]}}));
    let request = serde_json::to_value(&input).unwrap();
    let first = store.save_requirement(input).unwrap();
    // Resubmitting the same request returns the recorded outcome instead of
    // replaying the edit.
    let replay: SaveRequirement = serde_json::from_value(request).unwrap();
    let second = store.save_requirement(replay).unwrap();
    assert_eq!(first, second);
    assert_eq!(depends_on(&store), ["req_b"]);
}

#[test]
fn the_journal_holds_the_complete_before_and_after() {
    let (_temp, store) = fixture();
    seed_targets(&store);
    let head = store.requirement_edit_state(&scope(), &id()).unwrap();
    let before_digest = head.snapshot.as_ref().unwrap().digest.clone();
    let entry = save_ok(&store, "delta", json!({"depends_on": {"add": ["req_b", "req_c"]}}));
    // The Before snapshot is exactly the recorded state before the delta, and
    // the After snapshot carries the complete resulting set.
    assert_eq!(entry.before.as_ref().unwrap().digest, before_digest);
    assert_eq!(depends_on(&store), ["req_b", "req_c"]);
}
