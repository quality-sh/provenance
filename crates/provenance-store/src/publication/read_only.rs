use crate::layout::ProvenanceLayout;
use crate::state_store::StateStore;
use provenance_core::{NodeType, ScopeId};
use provenance_macros::rule;
use std::cell::RefCell;
use std::collections::BTreeSet;

thread_local! {
    static VALIDATIONS: RefCell<BTreeSet<String>> = const { RefCell::new(BTreeSet::new()) };
}

struct ReadOnlyValidation {
    key: String,
    inserted: bool,
}

impl Drop for ReadOnlyValidation {
    fn drop(&mut self) {
        if self.inserted {
            VALIDATIONS.with(|validations| validations.borrow_mut().remove(&self.key));
        }
    }
}

/// Bypasses publication writes for validation of an exclusively read state tree.
pub(crate) fn with_read_only_validation<R>(
    layout: &ProvenanceLayout,
    operation: impl FnOnce() -> anyhow::Result<R>,
) -> anyhow::Result<R> {
    let key = super::publication_lock_identity(layout)?;
    let inserted = VALIDATIONS.with(|validations| validations.borrow_mut().insert(key.clone()));
    let _validation = ReadOnlyValidation { key, inserted };
    operation()
}

pub(super) fn active(key: &str) -> bool {
    VALIDATIONS.with(|validations| validations.borrow().contains(key))
}

/// The record kinds that hold `id` in one scope, read with no lock and no
/// recovery. The caller gets the kinds and not a store, so nothing outside the
/// store crate reads or writes under the read-only marker.
#[rule("rule_read_only_section_stays_inside_the_store")]
pub fn read_only_record_kinds(
    layout: &ProvenanceLayout,
    scope: &ScopeId,
    id: &str,
) -> anyhow::Result<Vec<NodeType>> {
    let store = StateStore::new(layout.clone());
    with_read_only_validation(layout, || {
        let mut kinds = Vec::new();
        for kind in NodeType::ALL {
            if holds(&store, scope, kind, id)? {
                kinds.push(kind);
            }
        }
        Ok(kinds)
    })
}

fn holds(store: &StateStore, scope: &ScopeId, kind: NodeType, id: &str) -> anyhow::Result<bool> {
    macro_rules! any_id {
        ($list:ident) => {
            store
                .$list(scope)?
                .iter()
                .any(|record| record.id.as_str() == id)
        };
    }
    Ok(match kind {
        NodeType::Source => any_id!(list_sources),
        NodeType::Requirement => any_id!(list_requirements),
        NodeType::Resolution => any_id!(list_resolutions),
        NodeType::Rule => any_id!(list_rules),
        NodeType::Topic => any_id!(list_topics),
        NodeType::Question => any_id!(list_questions),
        NodeType::Domain => any_id!(list_domains),
        NodeType::Boundary => any_id!(list_boundaries),
    })
}
