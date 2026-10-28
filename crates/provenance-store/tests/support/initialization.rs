use provenance_store::{layout::ProvenanceLayout, repository_init::plan_manifest};

/// Uses the manifest plan that `provenance init --disposition-actor-id` uses.
#[allow(dead_code)]
pub fn initialize(layout: &ProvenanceLayout, actors: &[&str]) {
    let existing = layout
        .manifest_path()
        .exists()
        .then(|| serde_json::from_slice(&std::fs::read(layout.manifest_path()).unwrap()).unwrap());
    let manifest = plan_manifest(
        existing,
        Some("default"),
        None,
        actors.iter().map(|actor| (*actor).to_owned()).collect(),
        false,
        None,
    )
    .unwrap();
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
}

#[allow(dead_code)]
pub fn allow_reviewer(layout: &ProvenanceLayout) {
    initialize(layout, &["reviewer"]);
}
