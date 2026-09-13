use super::seeded_source_requirement_store;
use crate::state_store::CreateSourceInput;
use provenance_core::{SourceType, StableId};
use std::sync::mpsc;
use std::time::Duration;

fn replacement_source(
    scope_id: provenance_core::ScopeId,
    supersedes: StableId,
) -> CreateSourceInput {
    CreateSourceInput {
        scope_id,
        id: StableId::new("source_replacement").unwrap(),
        name: "Replacement policy".into(),
        source_type: SourceType::Policy,
        url: None,
        reference: None,
        commit_pin: None,
        effective_date: None,
        review_date: None,
        supersedes: vec![supersedes],
        origin_thread: None,
        origin_message: None,
    }
}

#[test]
fn source_creation_keeps_supersedes_validation_and_write_in_one_publication() {
    let (_dir, store, scope) = seeded_source_requirement_store();
    let older_id = StableId::new("source_schads").unwrap();
    let sources_path = crate::shards::sources_path(&store.layout, &scope);
    let (validated_tx, validated_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (writer_done_tx, writer_done_rx) = mpsc::channel();
    let writer = {
        let store = store.clone();
        let input = replacement_source(scope.clone(), older_id.clone());
        std::thread::spawn(move || {
            crate::test_probes::arm("source_supersedes_validated", move || {
                validated_tx.send(()).unwrap();
                release_rx
                    .recv_timeout(Duration::from_secs(5))
                    .map_err(anyhow::Error::from)
            });
            writer_done_tx.send(store.create_source(input)).unwrap();
        })
    };
    validated_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("source creation must reach the post-validation probe");

    let (publisher_done_tx, publisher_done_rx) = mpsc::channel();
    let publisher = {
        let store = store.clone();
        let scope = scope.clone();
        std::thread::spawn(move || {
            let result = store.with_repository_publication(|| {
                let mut sources = store.list_sources(&scope)?;
                sources.retain(|source| source.id != older_id);
                for source in &mut sources {
                    source.supersedes.retain(|target| target != &older_id);
                }
                crate::jsonl::write_jsonl_atomic(&sources_path, &sources)
            });
            publisher_done_tx.send(result).unwrap();
        })
    };

    let publisher_before_release = publisher_done_rx.recv_timeout(Duration::from_millis(300));
    let publication_waited = matches!(
        publisher_before_release,
        Err(mpsc::RecvTimeoutError::Timeout)
    );
    release_tx.send(()).unwrap();
    let created = writer_done_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("source creation must not deadlock")
        .unwrap();
    let publication_result = match publisher_before_release {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => publisher_done_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("publication must finish after source creation"),
        Err(error) => panic!("publication result channel failed: {error}"),
    };
    publication_result.unwrap();
    writer.join().unwrap();
    publisher.join().unwrap();

    let sources = store.list_sources(&scope).unwrap();
    let replacement = sources
        .iter()
        .find(|source| source.id == created.id)
        .expect("the replacement Source must survive publication");
    assert!(
        replacement.supersedes.is_empty(),
        "a successful create must not leave a dangling supersedes reference"
    );
    assert!(
        publication_waited,
        "publication must wait until source validation and mutation finish"
    );
}

#[test]
fn source_creation_refuses_an_initially_missing_supersedes_target_before_write() {
    let (_dir, store, scope) = seeded_source_requirement_store();
    let sources_path = crate::shards::sources_path(&store.layout, &scope);
    let before = std::fs::read(&sources_path).unwrap();

    let error = store
        .create_source(replacement_source(
            scope,
            StableId::new("source_missing").unwrap(),
        ))
        .unwrap_err();

    assert_eq!(
        error.to_string(),
        "source source_missing does not exist (--supersedes)"
    );
    assert_eq!(std::fs::read(sources_path).unwrap(), before);
}
