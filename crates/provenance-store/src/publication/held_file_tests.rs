//! Writes while a reader has a file of the state open. A reader such as a
//! second repository open, an editor or a virus scanner can have the
//! publication marker or a shard open when a write replaces it. On Linux and
//! macOS a rename always replaces an open file, so these tests can fail only
//! on Windows.

use super::*;
use crate::state_store::RecordStamp::ResolveFromRepository as StampFromGit;

/// This test has no Rule: no active Rule states that a write passes while a
/// reader has a file of the state open.
///
/// The test uses a private surface because no public call stops between two
/// writes of the publication marker, so only the probe can hold it open there.
///
/// The probe opens the publication marker when the publication has moved the
/// live state to its backup, and keeps the handle until the publication
/// returns. The next marker writes replace the open marker.
#[test]
fn a_publication_passes_while_a_reader_has_the_marker_open() {
    let directory = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from_path_buf(directory.path().to_path_buf()).unwrap();
    let layout = ProvenanceLayout::new(root.clone());
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    let manifest = provenance_core::Manifest::default_with_scope(
        provenance_core::ScopeId::new("default").unwrap(),
        provenance_core::RepoPathPrefix::new("."),
    );
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    let held = std::rc::Rc::new(std::cell::RefCell::new(None));
    crate::test_probes::arm("state_after_backup_rename", {
        let held = held.clone();
        let marker = layout.publication_marker_path();
        move || {
            held.replace(Some(std::fs::File::open(&marker)?));
            Ok(())
        }
    });
    let mut published = manifest;
    published.disposition_actor_ids = vec!["actor_after".to_owned()];
    let result = with_repository_publication(&layout, |held| {
        with_staged_state(held, false, StampFromGit, |staged| {
            let manifest = staged.layout.manifest_path();
            staged.publish_shard(&manifest, serde_json::to_vec(&published)?)
        })
    });
    crate::test_probes::disarm("state_after_backup_rename");
    let handle = held.take().expect("the publication reached the swap");
    result.unwrap_or_else(|error| panic!("{error:#}"));
    drop(handle);
    let repository = crate::repository::Repository::open(root.as_std_path(), "default");
    assert_eq!(repository.unwrap().manifest().unwrap(), published);
}

/// This test has no Rule: no active Rule states that a write passes while a
/// reader has a file of the state open.
///
/// A reader keeps the Contribution shard open while a public write of a
/// Contribution replaces it. The write publishes the shard as a one-key set
/// with no stage, so it replaces only that file.
#[test]
fn a_contribution_write_passes_while_a_reader_has_the_shard_open() {
    let directory = tempfile::tempdir().unwrap();
    let repository = crate::cache::tests::fixtures::initialized(directory.path());
    let store = repository.store();
    let scope = provenance_core::ScopeId::new("default").unwrap();
    store
        .create_requirement(
            serde_json::from_value(serde_json::json!({
                "scope_id": "default", "id": "req_a",
                "statement": "The system stores records.",
                "status": "discovery", "depends_on": [], "supersedes": [],
                "description": null, "refines": [],
                "spawned_by": null, "origin_thread": null, "origin_message": null
            }))
            .unwrap(),
        )
        .unwrap();
    let contribution = |id: &str| {
        serde_json::from_value(serde_json::json!({
            "scope_id": "default", "id": id,
            "target": {"artifact_type": "requirement", "artifact_id": "req_a"},
            "participant_slot": "reviewer", "stance": "support",
            "strongest_finding": "The records are stored.",
            "evidence_references": [], "material_claims": [], "risks": [],
            "objections": [], "challenges": [], "suggested_artifact_changes": [],
            "unsupported_recommendations": [],
            "uncertainty": {"level": "low", "rationale": "Direct."},
            "open_questions": []
        }))
        .unwrap()
    };
    store.create_contribution(contribution("contrib_a")).unwrap();
    let path = crate::shards::contributions_path(repository.layout(), &scope);
    let reader = std::fs::File::open(path).unwrap();
    store
        .create_contribution(contribution("contrib_b"))
        .unwrap_or_else(|error| panic!("{error:#}"));
    let read = crate::repository::Repository::open(directory.path(), "default")
        .unwrap()
        .store()
        .list_contributions(&scope)
        .unwrap();
    drop(reader);
    let ids: Vec<_> = read.iter().map(|record| record.id.as_str()).collect();
    assert_eq!(ids, ["contrib_a", "contrib_b"]);
}
