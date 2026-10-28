pub use provenance_transport::fixture::records::Repository;

#[path = "../../../provenance-store/tests/support/initialization.rs"]
mod initialization;

/// Uses the same manifest plan as repository initialization.
#[allow(dead_code)]
pub fn allow_reviewer(repo: &Repository) {
    initialization::allow_reviewer(&repo.layout);
}
