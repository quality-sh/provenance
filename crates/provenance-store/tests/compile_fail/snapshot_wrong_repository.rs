use provenance_store::layout::ProvenanceLayout;
use provenance_store::publication::{publication_guard, snapshot_state_under_guard};

async fn copy_another_repository() {
    let repository_a = ProvenanceLayout::new("repository_a");
    let repository_b = ProvenanceLayout::new("repository_b");
    let guard = publication_guard(&repository_a).await.unwrap();
    let _ = snapshot_state_under_guard(&guard, &repository_b);
}

fn main() {}
