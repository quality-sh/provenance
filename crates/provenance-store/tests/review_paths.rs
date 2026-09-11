#![cfg(any(unix, windows))]
mod review_support;

use camino::{Utf8Path, Utf8PathBuf};
use provenance_core::review::{EvidenceQuery, RequirementSnapshot, ReviewHistoryQuery};
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
    let first = store
        .save_requirement(save(&store, "enroll", json!({"description":"saved"})))
        .unwrap();
    let second = store
        .save_requirement(save(&store, "next", json!({"description":"later"})))
        .unwrap();
    drop(store);
    let reopened = StateStore::new(layout.clone());
    assert_eq!(
        reopened
            .requirement_save_receipt(&scope(), &id(), &second.request_id, "ben", None)
            .unwrap(),
        Some(second.clone())
    );
    cache::materialize_state(&layout).await.unwrap();
    let history = read_history(
        &root,
        &scope(),
        ReadPolicy::default(),
        ReviewHistoryQuery {
            requirement_id: id(),
            limit: 10,
            cursor: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(history.result.entries, [first.clone(), second]);
    let page = read_evidence(
        &root,
        &scope(),
        ReadPolicy::default(),
        EvidenceQuery {
            requirement_id: id(),
            entry_id: first.id,
            before: false,
            field: None,
            offset: 0,
        },
    )
    .await
    .unwrap();
    let snapshot: RequirementSnapshot = serde_json::from_str(&page.result.json_text).unwrap();
    assert_eq!(snapshot.record.description.as_deref(), Some("saved"));
}

fn review_dir(root: &Utf8Path) -> Utf8PathBuf {
    root.join(".provenance/state/scopes/default/review")
}

#[test]
fn receipt_rejects_an_internal_directory_escape() {
    let (temp, store) = fixture();
    let entry = store
        .save_requirement(save(&store, "enroll", json!({})))
        .unwrap();
    let root = Utf8Path::from_path(temp.path()).unwrap();
    let journal = review_dir(root).join("journal");
    let outside = tempfile::tempdir().unwrap();
    let moved = Utf8Path::from_path(outside.path()).unwrap().join("journal");
    std::fs::rename(&journal, &moved).unwrap();
    symlink_dir(&moved, &journal);
    assert!(store
        .requirement_save_receipt(&scope(), &id(), &entry.request_id, "ben", None)
        .is_err());
}
