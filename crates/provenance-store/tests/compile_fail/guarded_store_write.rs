use provenance_core::{ScopeId, StableId};
use provenance_store::publication::PublicationGuard;
use provenance_store::state_store::StateStore;

fn write_under_guard(guard: &PublicationGuard, scope: &ScopeId, id: &StableId) {
    let store = StateStore::under_guard(guard);
    let _ = store.clear_requirement_refines(scope, id);
}

fn main() {}
