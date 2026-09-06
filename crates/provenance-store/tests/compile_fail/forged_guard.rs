use provenance_store::layout::ProvenanceLayout;
use provenance_store::publication::PublicationGuard;
use provenance_store::state_store::StateStore;

fn main() {
    let layout = ProvenanceLayout::new("repo");
    let forged = PublicationGuard { _lock: None };
    let _ = StateStore::under_guard(&forged, layout);
}
