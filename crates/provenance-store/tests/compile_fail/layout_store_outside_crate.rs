use provenance_store::layout::ProvenanceLayout;
use provenance_store::state_store::StateStore;

fn main() {
    let _ = StateStore::new(ProvenanceLayout::new("repo"));
}
