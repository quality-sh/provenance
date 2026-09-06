use provenance_store::layout::ProvenanceLayout;
use provenance_store::publication::publication_guard;
use provenance_store::state_store::StateStore;

async fn read_after_guard_drops(layout: ProvenanceLayout) {
    let store = {
        let guard = publication_guard(&layout).await.unwrap();
        StateStore::under_guard(&guard)
    };
    let _ = store.manifest();
}

fn main() {}
