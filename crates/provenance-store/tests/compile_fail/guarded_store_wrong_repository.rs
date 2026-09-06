use provenance_store::layout::ProvenanceLayout;
use provenance_store::publication::publication_guard;
use provenance_store::state_store::StateStore;

async fn read_another_repository() {
    let repository_a = ProvenanceLayout::new("repository_a");
    let repository_b = ProvenanceLayout::new("repository_b");
    let guard = publication_guard(&repository_a).await.unwrap();
    let store = StateStore::under_guard(&guard, repository_b);
    let _ = store.manifest();
}

fn main() {}
