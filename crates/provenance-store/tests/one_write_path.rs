//! The one Requirement write path: the legacy authoring surfaces enroll
//! through the guarded journal, every write returns committed-success or a
//! typed error, and no surface advertises uncertainty.

mod review_support;
use provenance_core::review::{ReviewHistoryQuery, SaveOutcome};
use provenance_core::StableId;
use provenance_store::{
    cache,
    layout::ProvenanceLayout,
    operations::read_policy::ReadPolicy,
    review::{read_history, SaveRequirement},
    state_store::{CreateRequirementInput, StateStore},
    write_error::{WriteError, WriteFailure},
};
use review_support::*;
use serde_json::json;

fn new_requirement(id: &str) -> CreateRequirementInput {
    serde_json::from_value(json!({
        "scope_id": "default", "id": id,
        "statement": "The system stores records.",
        "status": "discovery", "depends_on": [], "supersedes": [],
        "description": null, "domain_id": null, "refines": null,
        "spawned_by": null, "origin_thread": null, "origin_message": null
    }))
    .unwrap()
}

#[tokio::test]
async fn the_legacy_create_enrolls_through_the_journal() {
    let (temp, store) = fixture();
    let created = store.create_requirement(new_requirement("req_b")).unwrap();
    assert_eq!(created.schema_version, provenance_core::review::REVIEW_SCHEMA_VERSION);

    // A repeated identical create resolves to the recorded outcome instead of
    // publishing a duplicate.
    let again = store.create_requirement(new_requirement("req_b")).unwrap();
    assert_eq!(again.id, created.id);
    assert_eq!(store.list_requirements(&scope()).unwrap().len(), 2);

    // A different identity on an existing record keeps the typed refusal.
    let error = store
        .create_requirement(serde_json::from_value({
            let mut value = serde_json::to_value(new_requirement("req_b")).unwrap();
            value["statement"] = json!("A different record.");
            value
        }).unwrap())
        .unwrap_err();
    assert!(matches!(
        WriteError(error).safe(),
        WriteFailure::AlreadyExists
    ));

    // The creation outcome is a journal entry with the authoring identity.
    let root = camino::Utf8Path::from_path(temp.path()).unwrap();
    let layout = ProvenanceLayout::new(root);
    cache::materialize_state(&layout).await.unwrap();
    let history = read_history(
        root,
        &scope(),
        ReadPolicy::default(),
        ReviewHistoryQuery {
            requirement_id: StableId::new("req_b").unwrap(),
            limit: 10,
            cursor: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(history.result.entries.len(), 1);
    assert_eq!(history.result.entries[0].outcome, SaveOutcome::Created);
    assert_eq!(history.result.entries[0].actor, "authoring");
}

#[test]
fn every_legacy_edit_publishes_a_journaled_outcome() {
    let (_temp, store) = fixture();
    store
        .create_source(serde_json::from_value(json!({
            "scope_id": "default", "id": "source_award", "name": "Award",
            "source_type": "policy", "url": null, "reference": "docs/award.md",
            "commit_pin": null, "effective_date": null, "review_date": null,
            "supersedes": [], "origin_thread": null, "origin_message": null
        }))
        .unwrap())
        .unwrap();
    let before = store.requirement_edit_state(&scope(), &id()).unwrap();

    store
        .update_requirement(serde_json::from_value(json!({
            "scope_id": "default", "id": "req_a", "description": "Edited"
        }))
        .unwrap())
        .unwrap();
    let after_text = store.requirement_edit_state(&scope(), &id()).unwrap();
    assert_ne!(after_text.etag, before.etag, "a text edit publishes");

    store
        .add_source_reference(serde_json::from_value(json!({
            "scope_id": "default", "source_id": "source_award",
            "requirement_id": "req_a", "clause": "4.2"
        }))
        .unwrap())
        .unwrap();
    let after_cite = store.requirement_edit_state(&scope(), &id()).unwrap();
    assert_ne!(after_cite.etag, after_text.etag, "a citation edit publishes");

    store
        .set_requirement_fog(&scope(), &id(), Some("Free text".into()))
        .unwrap();
    store
        .set_requirement_fog(&scope(), &id(), None)
        .unwrap();
    assert_eq!(store.list_requirements(&scope()).unwrap()[0].fog, None);

    // A self-refines cycle refuses, and the record keeps its earlier state.
    let before = store.list_requirements(&scope()).unwrap();
    assert!(store
        .set_requirement_refines(&scope(), &id(), StableId::new("req_a").unwrap())
        .is_err());
    assert_eq!(store.list_requirements(&scope()).unwrap(), before);
    store
        .clear_requirement_spawned_by(&scope(), &id())
        .unwrap();
}

#[test]
fn failed_edits_return_typed_errors_without_uncertainty() {
    let (_temp, store) = fixture();
    store
        .create_requirement(new_requirement("req_b"))
        .unwrap();
    store
        .set_requirement_refines(&scope(), &StableId::new("req_b").unwrap(), StableId::new("req_a").unwrap())
        .unwrap();

    // req_a refining req_b closes the cycle req_a -> req_b -> req_a: a typed
    // client failure, never an uncertain write.
    let error = store
        .set_requirement_refines(&scope(), &id(), StableId::new("req_b").unwrap())
        .unwrap_err();
    let failure = WriteError(error).safe();
    assert!(!matches!(failure, WriteFailure::UncertainWrite));
    assert!(
        matches!(failure, WriteFailure::InvalidUpdate),
        "{failure:?}"
    );

    // The store still answers after the refusal: the state is intact.
    let state = store.requirement_edit_state(&scope(), &id()).unwrap();
    assert!(state.snapshot.is_some());
}

#[test]
fn a_save_input_rejects_an_unknown_relationship_field() {
    let (_temp, store) = fixture();
    let input: Result<SaveRequirement, _> = serde_json::from_value(json!({
        "request_id": "delta", "actor": "ben",
        "expected_etag": store.requirement_edit_state(&scope(), &id()).unwrap().etag,
        "update": {"scope_id": "default", "id": "req_a"},
        "relationships": {"cites": {"add": [], "removes": []}}
    }));
    assert!(input.is_err(), "misspelled delta fields refuse to decode");
}
