use provenance_store::layout::ProvenanceLayout;
use provenance_store::publication::{snapshot_state_under_guard, PublicationGuard};

fn main() {
    let layout = ProvenanceLayout::new("repo");
    let forged = PublicationGuard { _lock: None };
    let _ = snapshot_state_under_guard(&forged, &layout);
}
