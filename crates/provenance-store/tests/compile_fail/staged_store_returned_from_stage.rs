use provenance_store::repository::Repository;
use provenance_store::state_store::StateStore;

fn main() {
    let repository = Repository::open("repo", "default").unwrap();
    let escaped = repository
        .stage(
            false,
            None,
            |staged: &StateStore| -> Result<&StateStore, anyhow::Error> { Ok(staged) },
        )
        .unwrap();
    let _ = escaped.manifest();
}
