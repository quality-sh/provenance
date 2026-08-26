use crate::layout::ProvenanceLayout;
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
pub fn with_read_only_validation<R>(
    layout: &ProvenanceLayout,
    operation: impl FnOnce() -> anyhow::Result<R>,
) -> anyhow::Result<R> {
    let key = layout.publication_lock_path().to_string();
    let inserted = VALIDATIONS.with(|validations| validations.borrow_mut().insert(key.clone()));
    let _validation = ReadOnlyValidation { key, inserted };
    operation()
}

pub(super) fn active(key: &str) -> bool {
    VALIDATIONS.with(|validations| validations.borrow().contains(key))
}
