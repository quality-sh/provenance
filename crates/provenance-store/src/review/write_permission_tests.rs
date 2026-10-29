//! A write permission travels with the staged store, also to a worker thread.

use super::{publication::with_record_state, CreateReviewRequirement};
use crate::{
    layout::ProvenanceLayout,
    shards,
    state_store::{RecordStamp, StateStore},
};
use camino::Utf8Path;
use provenance_core::{Requirement, ScopeId};
use provenance_macros::verifies;
use serde_json::json;

const CHANGED: &str = "The system keeps records.";

fn scope() -> ScopeId {
    ScopeId::new("default").unwrap()
}

/// A repository with one enrolled Requirement, `req_a`.
fn fixture() -> (tempfile::TempDir, StateStore) {
    let temp = tempfile::tempdir().unwrap();
    let layout = ProvenanceLayout::new(Utf8Path::from_path(temp.path()).unwrap());
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    std::fs::write(
        layout.manifest_path(),
        r#"{"schema_version":2,"scopes":[{"id":"default","path_prefix":"."}]}"#,
    )
    .unwrap();
    let store = crate::test_support::store(&layout);
    let create: CreateReviewRequirement = serde_json::from_value(json!({
        "actor":"ben", "origin":null,
        "create":{"scope_id":"default","id":"req_a","statement":"The system stores records.",
            "status":"discovery","depends_on":[],"supersedes":[]}
    }))
    .unwrap();
    store.create_review_requirement(create).unwrap();
    (temp, store)
}

fn restate(store: &StateStore, path: &Utf8Path) -> anyhow::Result<()> {
    store.mutate_jsonl_records(path, |records: &mut Vec<Requirement>| {
        records[0].statement = CHANGED.to_owned();
        Ok(())
    })
}

/// Runs `write` on a worker thread with the staged store of one review write.
fn staged_worker(
    store: &StateStore,
    write: impl FnOnce(&StateStore, &Utf8Path) -> anyhow::Result<()> + Send,
) -> anyhow::Result<()> {
    store.with_repository_publication(|| {
        with_record_state(store, RecordStamp::Omit, |staged| {
            let path = shards::requirements_path(&staged.layout, &scope());
            std::thread::scope(|threads| threads.spawn(|| write(staged, &path)).join().unwrap())
        })
    })
}

#[test]
#[verifies("rule_enrolled_record_write_needs_store_permission", examples)]
fn a_worker_saves_an_enrolled_requirement_through_the_permitted_store() {
    let (_temp, store) = fixture();
    staged_worker(&store, |staged, path| {
        staged.permitting(&[path], "req_a", |permitted| restate(permitted, path))
    })
    .unwrap();
    assert_eq!(
        store.list_requirements(&scope()).unwrap()[0].statement,
        CHANGED
    );
}

#[test]
#[verifies("rule_enrolled_record_write_needs_store_permission", examples)]
fn an_unpermitted_staged_write_is_refused_and_publishes_nothing() {
    let (_temp, store) = fixture();
    let path = shards::requirements_path(&store.layout, &scope());
    let before = std::fs::read(&path).unwrap();
    let refused = staged_worker(&store, restate).unwrap_err();
    assert!(format!("{refused:#}").contains("requires a guarded review save"));
    let other_record = staged_worker(&store, |staged, path| {
        staged.permitting(&[path], "req_b", |permitted| restate(permitted, path))
    })
    .unwrap_err();
    assert!(format!("{other_record:#}").contains("requires a guarded review save"));
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
#[verifies("rule_enrolled_record_write_needs_store_permission", examples)]
fn raw_and_live_writes_of_an_enrolled_requirement_are_refused() {
    let (_temp, store) = fixture();
    let path = shards::requirements_path(&store.layout, &scope());
    let before = std::fs::read(&path).unwrap();
    let mut record = serde_json::to_value(&store.list_requirements(&scope()).unwrap()[0]).unwrap();
    record["statement"] = json!(CHANGED);
    assert!(crate::jsonl::write_jsonl_atomic(&path, &[record]).is_err());
    assert!(store
        .permitting(&[&path], "req_a", |live| restate(live, &path))
        .is_err());
    assert!(restate(&store, &path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
}
