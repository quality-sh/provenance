use provenance_core::{ScopeId, StableId};
use provenance_store::publication::PublicationGuard;
use provenance_store::state_store::StateStore;

fn write_under_guard(guard: &PublicationGuard, scope: &ScopeId, id: &StableId) {
    let store = StateStore::under_guard(guard);
    let _ = store.set_requirement_fog(scope, id, Some("Changed".into()));
}

fn main() {}
