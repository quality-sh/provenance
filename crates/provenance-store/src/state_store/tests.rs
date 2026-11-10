#[cfg(feature = "host")]
use crate::{
    layout::ProvenanceLayout,
    state_store::{CreateRequirementInput, CreateSourceInput, StateStore},
};
#[cfg(feature = "host")]
use provenance_core::{Manifest, RepoPathPrefix, RequirementStatus, ScopeId, SourceType, StableId};

#[cfg(feature = "host")]
fn initialized_store() -> (tempfile::TempDir, StateStore, ScopeId) {
    let dir = tempfile::tempdir().unwrap();
    let root = camino::Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
    let (store, scope) = initialized_store_in(root);
    (dir, store, scope)
}

#[cfg(feature = "host")]
fn initialized_store_in(root: camino::Utf8PathBuf) -> (StateStore, ScopeId) {
    let layout = ProvenanceLayout::new(root);
    std::fs::create_dir_all(layout.manifest_path().parent().unwrap()).unwrap();
    let scope = ScopeId::new("default").unwrap();
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_string(&Manifest::default_with_scope(
            scope.clone(),
            RepoPathPrefix::new("."),
        ))
        .unwrap(),
    )
    .unwrap();
    let store = crate::test_support::store(&layout);
    (store, scope)
}

#[cfg(feature = "host")]
fn staged_store() -> (tempfile::TempDir, StateStore, StateStore, ScopeId, Vec<u8>) {
    let (dir, live_store, scope) = initialized_store();
    let manifest_before = std::fs::read(live_store.layout.manifest_path()).unwrap();
    let staged_root = camino::Utf8PathBuf::from_path_buf(dir.path().join("staged-repo")).unwrap();
    let staged_layout = ProvenanceLayout::new(staged_root);
    std::fs::create_dir_all(staged_layout.state_dir()).unwrap();
    std::fs::write(staged_layout.manifest_path(), &manifest_before).unwrap();
    let staged_store = StateStore::staged_view(
        None,
        staged_layout,
        super::RecordStamp::ResolveFromRepository,
    );
    (dir, live_store, staged_store, scope, manifest_before)
}

#[cfg(feature = "host")]
fn seeded_requirement_store() -> (tempfile::TempDir, StateStore, ScopeId) {
    let (dir, store, scope) = initialized_store();
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
    (dir, store, scope)
}

#[cfg(feature = "host")]
pub(super) fn seeded_source_requirement_store() -> (tempfile::TempDir, StateStore, ScopeId) {
    let (dir, store, scope) = seeded_requirement_store();
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
    (dir, store, scope)
}

mod asserted_evidence;
mod proposal_surfacing;

#[cfg(feature = "host")]
mod archive_history;
#[cfg(feature = "host")]
mod direct_statement_write_gate;
#[cfg(feature = "host")]
mod graph_validation;
#[cfg(feature = "host")]
mod ideation;
#[cfg(feature = "host")]
mod ideation_duplicates;
#[cfg(feature = "host")]
mod ideation_targets;
#[cfg(feature = "host")]
mod implementation_bindings;
#[cfg(feature = "host")]
mod legacy_coexistence;
#[cfg(feature = "host")]
mod message_ids;
#[cfg(feature = "host")]
mod native_review_writer_findings;
#[cfg(feature = "host")]
mod native_review_writers;
#[cfg(feature = "host")]
mod native_rule_creation;
#[cfg(feature = "host")]
mod proposal_surfaces;
#[cfg(feature = "host")]
mod proposals;
#[cfg(feature = "host")]
mod read_budget;
#[cfg(feature = "host")]
mod read_budget_boundaries;
#[cfg(feature = "host")]
mod references;
#[cfg(feature = "host")]
mod reserved_ids;
#[cfg(feature = "host")]
mod scope_import_access;
#[cfg(feature = "host")]
mod scope_ingestion;
#[cfg(feature = "host")]
mod scope_ingestion_order;
#[cfg(feature = "host")]
mod shaping;
#[cfg(feature = "host")]
mod shaping_archive_atomicity;
#[cfg(feature = "host")]
mod source_requirements;
#[cfg(feature = "host")]
mod source_supersedes_concurrency;
#[cfg(feature = "host")]
mod threads;
#[cfg(feature = "host")]
mod typed_adoption;
#[cfg(feature = "host")]
mod typed_decision;
#[cfg(feature = "host")]
mod typed_references;
#[cfg(feature = "host")]
mod typed_review_writers;
#[cfg(feature = "host")]
mod typed_statement_feedback;
#[cfg(feature = "host")]
mod verification_bindings;

#[cfg(feature = "host")]
mod requirement_archive;
#[cfg(feature = "host")]
mod requirement_archive_children;
#[cfg(feature = "host")]
mod requirement_archive_links;
#[cfg(all(windows, feature = "host"))]
mod requirement_archive_long_path;
#[cfg(feature = "host")]
mod requirement_archive_parents;
