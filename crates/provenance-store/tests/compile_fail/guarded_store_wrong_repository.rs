use provenance_store::publication::publication_guard;
use provenance_store::repository::Repository;
use provenance_store::state_store::StateStore;

async fn read_another_repository() {
    let repository_a = Repository::open("repository_a", "default").unwrap();
    let repository_b = Repository::open("repository_b", "default").unwrap();
    let guard = publication_guard(&repository_a).await.unwrap();
    let store = StateStore::under_guard(&guard, repository_b);
    let _ = store.manifest();
}

fn main() {}
