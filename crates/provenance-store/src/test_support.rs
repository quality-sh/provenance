#[path = "../tests/support/initialization.rs"]
mod initialization;
pub use initialization::allow_reviewer;

/// The fixture repository at `layout`, opened for `default`, or for the
/// first scope its manifest declares. The open runs publication recovery
/// before it reads the manifest.
pub fn repository(layout: &crate::layout::ProvenanceLayout) -> crate::repository::Repository {
    use crate::repository::{OpenFailure, Repository};
    match Repository::open(layout.root(), "default") {
        Ok(repository) => repository,
        Err(OpenFailure::UnknownScope { declared, .. }) => {
            Repository::open(layout.root(), &declared[0]).unwrap()
        }
        Err(error) => panic!("open the fixture repository {}: {error}", layout.root()),
    }
}

/// A store over the fixture repository at `layout`.
pub fn store(layout: &crate::layout::ProvenanceLayout) -> crate::state_store::StateStore {
    repository(layout).store()
}
