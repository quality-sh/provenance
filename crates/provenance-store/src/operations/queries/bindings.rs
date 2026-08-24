use provenance_core::{ImplementationBinding, ScopeId, VerificationBinding};
use provenance_store::state_store::StateStore;

/// The canonical relationships from code to Rules, in one settled order.
///
/// Active views leave retired bindings out; the flag is the only way to see
/// them, and it reads the same way in every primitive.
pub(super) struct Bindings {
    pub implementations: Vec<ImplementationBinding>,
    pub verifications: Vec<VerificationBinding>,
}

impl Bindings {
    pub(super) fn load(
        store: &StateStore,
        scope: &ScopeId,
        include_retired: bool,
    ) -> anyhow::Result<Self> {
        let mut implementations = store.list_implementation_bindings(scope)?;
        implementations.retain(|binding| include_retired || !binding.retired);
        implementations.sort_by(|left, right| left.id.as_str().cmp(right.id.as_str()));
        let mut verifications = store.list_verification_bindings(scope)?;
        verifications.retain(|binding| include_retired || !binding.retired);
        verifications.sort_by(|left, right| left.id.as_str().cmp(right.id.as_str()));
        Ok(Self {
            implementations,
            verifications,
        })
    }
}
