#![cfg(any(unix, windows))]
#[allow(dead_code)]
mod review_support;

use camino::Utf8Path;
use provenance_core::review::{EvidenceQuery, ReviewHistoryQuery};
use provenance_store::{
    cache,
    layout::ProvenanceLayout,
    operations::read_policy::ReadPolicy,
    review::{read_evidence, read_history},
    state_store::StateStore,
};
use review_support::*;
use serde_json::json;

fn symlink_dir(target: &Utf8Path, link: &Utf8Path) {
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, link).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_dir(target, link).unwrap();
}

#[tokio::test]
async fn review_reads_and_projection_accept_a_symlinked_repository_parent() {
    let (temp, store) = fixture();
    drop(store);
    let aliases = tempfile::tempdir().unwrap();
    let physical = Utf8Path::from_path(temp.path()).unwrap();
    let alias = Utf8Path::from_path(aliases.path()).unwrap().join("parent");
    symlink_dir(physical.parent().unwrap(), &alias);
    let root = alias.join(physical.file_name().unwrap());
    assert!(std::fs::symlink_metadata(&alias)
        .unwrap()
        .file_type()
        .is_symlink());
    let layout = ProvenanceLayout::new(&root);
    let store = StateStore::new(layout.clone());
    store
        .save_requirement(save(&store, json!({"description":"saved"})))
        .unwrap();
    cache::materialize_state(&layout).await.unwrap();
    let history = read_history(
        &root,
        &scope(),
        ReadPolicy::default(),
        ReviewHistoryQuery {
            record_kind: provenance_core::NodeType::Requirement,
            record_id: id(),
            limit: 10,
            cursor: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(history.result.entries.len(), 1);
    let working = history.result.entries[0].id.clone();
    assert_eq!(working.as_str(), "working");
    let page = read_evidence(
        &root,
        &scope(),
        ReadPolicy::default(),
        EvidenceQuery {
            record_kind: provenance_core::NodeType::Requirement,
            record_id: id(),
            entry_id: working,
            before: false,
            field: Some("description".into()),
            offset: 0,
        },
    )
    .await
    .unwrap();
    assert_eq!(page.result.json_text, "\"saved\"");
}
