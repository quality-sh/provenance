use provenance_store::publication::publication_guard;
use provenance_store::repository::Repository;
use provenance_store::state_store::StateStore;

async fn read_after_guard_drops(repository: Repository) {
    let store = {
        let guard = publication_guard(&repository).await.unwrap();
        StateStore::under_guard(&guard)
    };
    let _ = store.manifest();
}

fn main() {}
