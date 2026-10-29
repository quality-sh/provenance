use provenance_store::publication::PublicationGuard;
use provenance_store::repository::Repository;
use provenance_store::state_store::StateStore;

fn main() {
    let repository = Repository::open("repo", "default").unwrap();
    let forged = PublicationGuard {
        _lock: None,
        repository,
    };
    let _ = StateStore::under_guard(&forged);
}
