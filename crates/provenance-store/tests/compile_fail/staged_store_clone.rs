use provenance_store::repository::Repository;
use provenance_store::state_store::StateStore;

fn main() {
    let repository = Repository::open("repo", "default").unwrap();
    let _ = repository.stage(false, None, |staged| Ok(StateStore::clone(staged)));
}
