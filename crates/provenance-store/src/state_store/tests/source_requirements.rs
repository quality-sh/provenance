use super::{initialized_store, seeded_requirement_store, seeded_source_requirement_store};
use crate::state_store::{AddSourceReferenceInput, CreateRequirementInput, CreateSourceInput};
use provenance_core::{EdgeType, RequirementStatus, SourceType, StableId};

#[test]
fn source_requirement_records_are_written_deterministically() {
    let (_dir, store, scope) = initialized_store();
    store
        .create_source(CreateSourceInput {
            scope_id: scope.clone(),
            id: StableId::new("source_schads").unwrap(),
            name: "SCHADS Award".into(),
            source_type: SourceType::Policy,
            url: None,
            reference: None,
            commit_pin: None,
            effective_date: None,
            review_date: None,
            supersedes: Vec::new(),
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
    store
        .create_requirement(CreateRequirementInput {
            scope_id: scope.clone(),
            id: StableId::new("req_overtime").unwrap(),
            statement: "Overtime".into(),
            description: None,
            status: RequirementStatus::Active,
            domain_id: None,
            refines: None,
            depends_on: Vec::new(),
            supersedes: Vec::new(),
            spawned_by: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
    store
        .add_source_reference(AddSourceReferenceInput {
            scope_id: scope.clone(),
            source_id: StableId::new("source_schads").unwrap(),
            requirement_id: StableId::new("req_overtime").unwrap(),
            clause: None,
        })
        .unwrap();
    assert_eq!(
        store.list_sources(&scope).unwrap()[0].id.as_str(),
        "source_schads"
    );
    assert_eq!(
        store.list_edges().unwrap()[0].edge_type,
        EdgeType::References
    );
}

#[test]
fn canonical_state_readers_tolerate_unknown_extension_fields() {
    let (_dir, store, scope) = seeded_source_requirement_store();
    let source_path = store
        .layout
        .scopes_dir()
        .join(scope.as_str())
        .join("sources/source.jsonl");
    let source = std::fs::read_to_string(&source_path).unwrap();
    std::fs::write(
        source_path,
        source.replace(
            "\"name\":\"SCHADS Award\"",
            "\"name\":\"SCHADS Award\",\"extension\":true",
        ),
    )
    .unwrap();

    assert_eq!(store.list_sources(&scope).unwrap()[0].name, "SCHADS Award");
}

#[test]
fn concurrent_source_creates_preserve_all_records() {
    let (_dir, store, scope) = initialized_store();

    for index in 0..200 {
        store
            .create_source(CreateSourceInput {
                scope_id: scope.clone(),
                id: StableId::new(format!("source_seed_{index:03}")).unwrap(),
                name: format!("Seed {index:03}"),
                source_type: SourceType::Policy,
                url: None,
                reference: None,
                commit_pin: None,
                effective_date: None,
                review_date: None,
                supersedes: Vec::new(),
                origin_thread: None,
                origin_message: None,
            })
            .unwrap();
    }

    let writer_count = 16;
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(writer_count));
    let mut handles = Vec::new();
    for index in 0..writer_count {
        let store = store.clone();
        let scope = scope.clone();
        let barrier = barrier.clone();
        handles.push(std::thread::spawn(move || {
            barrier.wait();
            store
                .create_source(CreateSourceInput {
                    scope_id: scope,
                    id: StableId::new(format!("source_concurrent_{index:03}")).unwrap(),
                    name: format!("Concurrent {index:03}"),
                    source_type: SourceType::Policy,
                    url: None,
                    reference: None,
                    commit_pin: None,
                    effective_date: None,
                    review_date: None,
                    supersedes: Vec::new(),
                    origin_thread: None,
                    origin_message: None,
                })
                .unwrap();
        }));
    }
    for handle in handles {
        handle.join().unwrap();
    }

    let sources = store.list_sources(&scope).unwrap();
    assert_eq!(sources.len(), 200 + writer_count);
    for index in 0..writer_count {
        assert!(sources
            .iter()
            .any(|source| { source.id.as_str() == format!("source_concurrent_{index:03}") }));
    }
}

#[test]
fn repository_publication_cannot_overwrite_a_concurrent_writer() {
    let (_dir, store, scope) = initialized_store();
    let (publication_ready_tx, publication_ready_rx) = std::sync::mpsc::channel();
    let (publish_tx, publish_rx) = std::sync::mpsc::channel();
    let publisher = {
        let store = store.clone();
        let scope = scope.clone();
        std::thread::spawn(move || {
            store
                .with_repository_publication(|| {
                    publication_ready_tx.send(()).unwrap();
                    publish_rx.recv().unwrap();
                    crate::jsonl::write_jsonl_atomic::<provenance_core::Source>(
                        &crate::shards::sources_path(&store.layout, &scope),
                        &[],
                    )
                })
                .unwrap();
        })
    };
    publication_ready_rx.recv().unwrap();

    let (writer_done_tx, writer_done_rx) = std::sync::mpsc::channel();
    let writer = {
        let store = store.clone();
        let scope = scope.clone();
        std::thread::spawn(move || {
            store
                .create_source(CreateSourceInput {
                    scope_id: scope,
                    id: StableId::new("source_after_snapshot").unwrap(),
                    name: "After snapshot".into(),
                    source_type: SourceType::Policy,
                    url: None,
                    reference: None,
                    commit_pin: None,
                    effective_date: None,
                    review_date: None,
                    supersedes: Vec::new(),
                    origin_thread: None,
                    origin_message: None,
                })
                .unwrap();
            writer_done_tx.send(()).unwrap();
        })
    };
    assert!(writer_done_rx
        .recv_timeout(std::time::Duration::from_millis(100))
        .is_err());
    publish_tx.send(()).unwrap();
    publisher.join().unwrap();
    writer.join().unwrap();

    assert_eq!(
        store.list_sources(&scope).unwrap()[0].id.as_str(),
        "source_after_snapshot"
    );
}

#[test]
fn repository_publication_excludes_an_entire_multi_shard_write() {
    let (_dir, store, scope) = seeded_source_requirement_store();
    let (publication_ready_tx, publication_ready_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let publisher = {
        let store = store.clone();
        std::thread::spawn(move || {
            store
                .with_repository_publication(|| {
                    publication_ready_tx.send(()).unwrap();
                    release_rx.recv().unwrap();
                    Ok(())
                })
                .unwrap();
        })
    };
    publication_ready_rx.recv().unwrap();

    let (writer_done_tx, writer_done_rx) = std::sync::mpsc::channel();
    let writer = {
        let store = store.clone();
        let scope = scope.clone();
        std::thread::spawn(move || {
            store
                .add_source_reference(AddSourceReferenceInput {
                    scope_id: scope,
                    source_id: StableId::new("source_schads").unwrap(),
                    requirement_id: StableId::new("req_overtime").unwrap(),
                    clause: Some("clause 1".into()),
                })
                .unwrap();
            writer_done_tx.send(()).unwrap();
        })
    };
    assert!(writer_done_rx
        .recv_timeout(std::time::Duration::from_millis(100))
        .is_err());
    release_tx.send(()).unwrap();
    publisher.join().unwrap();
    writer.join().unwrap();

    assert_eq!(store.list_edges().unwrap().len(), 1);
    assert_eq!(
        store.list_requirements(&scope).unwrap()[0].source_refs,
        vec![provenance_core::SourceReference {
            source_id: StableId::new("source_schads").unwrap(),
            clause: Some("clause 1".into()),
        }]
    );
}

#[test]
fn requirement_fog_is_set_and_cleared_as_free_text() {
    let (_dir, store, scope) = seeded_requirement_store();
    let requirement_id = StableId::new("req_overtime").unwrap();

    let updated = store
        .set_requirement_fog(
            &scope,
            &requirement_id,
            Some("something about public holidays and sleepovers".into()),
        )
        .unwrap();
    assert_eq!(
        updated.fog.as_deref(),
        Some("something about public holidays and sleepovers")
    );
    assert_eq!(
        store.list_requirements(&scope).unwrap()[0].fog.as_deref(),
        Some("something about public holidays and sleepovers")
    );

    let cleared = store
        .set_requirement_fog(&scope, &requirement_id, None)
        .unwrap();
    assert_eq!(cleared.fog, None);
    assert!(store
        .set_requirement_fog(&scope, &StableId::new("req_missing").unwrap(), None)
        .unwrap_err()
        .to_string()
        .contains("requirement does not exist"));
}
