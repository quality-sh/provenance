use crate::{
    layout::ProvenanceLayout,
    state_store::{CreateRequirementInput, CreateSourceInput, StateStore},
};
use provenance_core::{Manifest, RepoPathPrefix, RequirementStatus, ScopeId, SourceType, StableId};

fn initialized_store() -> (tempfile::TempDir, StateStore, ScopeId) {
    let dir = tempfile::tempdir().unwrap();
    let root = camino::Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
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
    let store = StateStore::new(layout);
    (dir, store, scope)
}

fn seeded_requirement_store() -> (tempfile::TempDir, StateStore, ScopeId) {
    let (dir, store, scope) = initialized_store();
    store
        .create_requirement(CreateRequirementInput {
            scope_id: scope.clone(),
            id: StableId::new("req_overtime").unwrap(),
            statement: "Overtime".into(),
            description: None,
            status: RequirementStatus::Active,
            domain_id: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
    (dir, store, scope)
}

fn seeded_source_requirement_store() -> (tempfile::TempDir, StateStore, ScopeId) {
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
            effective_date: None,
            review_date: None,
            superseded_by: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
    (dir, store, scope)
}

mod asserted_evidence;
mod direct_statement_write_gate;
mod edges;
mod ideation;
mod ideation_duplicates;
mod implementation_bindings;
mod legacy_coexistence;
mod manifest;
mod proposal_surfaces;
mod proposals;
mod shaping;
mod source_requirements;
mod threads;
mod typed_adoption;
mod typed_statement_feedback;
mod verification_bindings;
