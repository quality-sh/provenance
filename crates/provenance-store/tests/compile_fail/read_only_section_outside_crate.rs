use provenance_store::layout::ProvenanceLayout;
use provenance_store::publication::with_read_only_validation;

fn main() {
    let _ = with_read_only_validation(&ProvenanceLayout::new("repo"), || Ok(()));
}
