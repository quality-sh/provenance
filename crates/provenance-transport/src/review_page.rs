//! A trusted page-write mark. HTTP clients cannot supply this request extension.
use provenance_store::{
    layout::ProvenanceLayout,
    publication::with_publication_guard,
    review_changes::{ReviewChanges, ReviewStateSnapshot},
};
use std::sync::Arc;

/// The owning host adds this mark after it authenticates the page's session cookie.
#[derive(Clone)]
pub struct ReviewPageWrite;

#[derive(Clone)]
pub struct PageWrites {
    pub layout: ProvenanceLayout,
    pub observed: Arc<dyn Fn(ReviewChanges) + Send + Sync>,
}

impl PageWrites {
    pub fn run<R>(&self, work: impl FnOnce() -> R) -> anyhow::Result<R> {
        with_publication_guard(&self.layout, |guard| {
            let before = ReviewStateSnapshot::read(guard)?;
            let result = work();
            let after = ReviewStateSnapshot::read(guard)?;
            let _ = (&before, &after);
            (self.observed)(ReviewChanges { configuration_changed: true, scopes: vec![] });
            Ok(result)
        })
    }
}
