use super::{initialized_store, seeded_requirement_store, seeded_source_requirement_store};
use crate::state_store::{AddSourceReferenceInput, CreateRequirementInput, CreateSourceInput};
use provenance_core::{RequirementStatus, SourceType, StableId};

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
            refines: Vec::new(),
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
        store.list_requirements(&scope).unwrap()[0].source_refs[0]
            .source_id
            .as_str(),
        "source_schads"
    );
}

#[test]
fn concurrent_source_creates_preserve_all_records() {
    let (_dir, store, scope) = initialized_store();
    let repository = crate::test_support::repository(&store.layout);

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
        let store = repository.store();
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
    let repository = crate::test_support::repository(&store.layout);
    let (publication_ready_tx, publication_ready_rx) = std::sync::mpsc::channel();
    let (publish_tx, publish_rx) = std::sync::mpsc::channel();
    let publisher = {
        let store = repository.store();
        let scope = scope.clone();
        std::thread::spawn(move || {
            store
                .with_repository_publication(|_| {
                    publication_ready_tx.send(()).unwrap();
                    publish_rx.recv().unwrap();
                    crate::jsonl::write_jsonl_atomic_under_publication::<provenance_core::Source>(
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
        let store = repository.store();
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
    let repository = crate::test_support::repository(&store.layout);
    let (publication_ready_tx, publication_ready_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let publisher = {
        let store = repository.store();
        std::thread::spawn(move || {
            store
                .with_repository_publication(|_| {
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
        let store = repository.store();
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

    assert_eq!(
        store.list_requirements(&scope).unwrap()[0].source_refs,
        vec![provenance_core::SourceReference {
            source_id: StableId::new("source_schads").unwrap(),
            clause: Some("clause 1".into()),
        }]
    );
}
