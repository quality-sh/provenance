#[path = "../tests/support/initialization.rs"]
mod initialization;
pub use initialization::allow_reviewer;

/// A store over the fixture repository at `layout`, from a `Repository`
/// opened for the first scope its manifest declares.
pub fn store(layout: crate::layout::ProvenanceLayout) -> crate::state_store::StateStore {
    let manifest = std::fs::read(layout.manifest_path()).unwrap_or_else(|error| {
        panic!("a fixture writes {} first: {error}", layout.manifest_path())
    });
    let manifest: provenance_core::Manifest = serde_json::from_slice(&manifest).unwrap();
    crate::repository::Repository::open(layout.root(), manifest.scopes[0].id.as_str())
        .unwrap()
        .store()
}
