use crate::{layout::ProvenanceLayout, state_store::StateStore};
use provenance_core::{Manifest, RepoPathPrefix, ScopeId};

/// Configures the shared test reviewer through the typed manifest model.
pub fn allow_reviewer(layout: &ProvenanceLayout) {
    std::fs::create_dir_all(layout.manifest_path().parent().unwrap()).unwrap();
    let mut manifest = if layout.manifest_path().exists() {
        StateStore::new(layout.clone()).manifest().unwrap()
    } else {
        Manifest::default_with_scope(ScopeId::new("default").unwrap(), RepoPathPrefix::new("."))
    };
    if !manifest
        .disposition_actor_ids
        .iter()
        .any(|actor| actor == "reviewer")
    {
        manifest.disposition_actor_ids.push("reviewer".into());
    }
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
}
