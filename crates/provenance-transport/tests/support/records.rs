pub use provenance_transport::fixture::records::Repository;

/// Configures the reviewer through the repository configuration API.
#[allow(dead_code)]
pub fn allow_reviewer(repo: &Repository) {
    provenance_store::state_store::StateStore::new(repo.layout.clone())
        .set_disposition_actor_ids(vec!["reviewer".into()])
        .unwrap();
}
