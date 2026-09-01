use super::super::*;
use super::fixtures::*;
use crate::publication::events_in_window;
use crate::state_store::StateStore;
use provenance_core::ScopeId;

#[test]
fn every_declared_shard_path_maps_back_to_its_family() {
    let (_dir, layout, scope) = empty_layout();
    for family in ProjectionFamily::ALL {
        let shard_scope = family.is_scoped().then_some(&scope);
        let path = family.shard_path(&layout, shard_scope).unwrap();
        let (mapped, mapped_scope) = family_for_shard_path(&layout, &path)
            .unwrap_or_else(|| panic!("no mapping for {}", family.family_name()));
        assert_eq!(mapped, family, "path {path} must map by declared family");
        assert_eq!(mapped_scope.as_ref(), shard_scope, "{path}");
    }
}

#[test]
fn sibling_files_in_one_directory_map_to_their_own_families() {
    let (_dir, layout, scope) = empty_layout();
    let reviews = crate::shards::requirement_reviews_path(&layout, &scope);
    let requirements = crate::shards::requirements_path(&layout, &scope);
    let messages = crate::shards::messages_path(&layout, &scope);
    let threads = crate::shards::threads_path(&layout, &scope);
    assert_eq!(
        family_for_shard_path(&layout, &reviews).unwrap().0,
        ProjectionFamily::RequirementReviews
    );
    assert_eq!(
        family_for_shard_path(&layout, &requirements).unwrap().0,
        ProjectionFamily::Requirements
    );
    assert_eq!(
        family_for_shard_path(&layout, &messages).unwrap().0,
        ProjectionFamily::Messages
    );
    assert_eq!(
        family_for_shard_path(&layout, &threads).unwrap().0,
        ProjectionFamily::Threads
    );
}

#[test]
fn a_path_outside_the_family_table_maps_to_nothing() {
    let (_dir, layout, _scope) = empty_layout();
    assert!(family_for_shard_path(&layout, &layout.manifest_path()).is_none());
    let stray = layout
        .scopes_dir()
        .join("default")
        .join("requirements/notes.jsonl");
    assert!(family_for_shard_path(&layout, &stray).is_none());
}

#[test]
fn committed_writes_leave_journal_events_named_by_family() {
    let (_dir, layout, scope) = empty_layout();
    let store = StateStore::new(layout.clone());
    create_source(&store, &scope, "source_journal");

    let events = events_in_window(&layout, 1, i64::MAX).unwrap();
    assert!(
        events
            .iter()
            .any(|event| event.family == "sources" && event.scope == scope.as_str()),
        "a source write must journal a sources event: {events:?}"
    );
    let mut sequences: Vec<i64> = events.iter().map(|event| event.sequence).collect();
    let sorted = sequences.clone();
    sequences.sort_unstable();
    assert_eq!(sequences, sorted, "sequences allocate in order");
}

#[test]
fn a_crash_before_the_journal_append_loses_only_the_hint() {
    let (_dir, layout, scope) = empty_layout();
    let store = StateStore::new(layout.clone());
    crate::test_probes::crash_at("writer_canonical_committed");
    let error = ScopeId::new("default")
        .and_then(|_| {
            store.create_source(crate::state_store::CreateSourceInput {
                scope_id: scope.clone(),
                id: sid("source_crashed"),
                name: "Crashed".into(),
                source_type: provenance_core::SourceType::Policy,
                url: None,
                reference: None,
                commit_pin: None,
                effective_date: None,
                review_date: None,
                superseded_by: None,
                origin_thread: None,
                origin_message: None,
            })
        })
        .unwrap_err();
    crate::test_probes::disarm("writer_canonical_committed");
    assert!(error.to_string().contains("injected crash"), "{error}");

    // The canonical write survived; the journal never heard about it.
    assert_eq!(store.list_sources(&scope).unwrap().len(), 1);
    assert!(events_in_window(&layout, 1, i64::MAX).unwrap().is_empty());
}
