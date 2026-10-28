//! A trusted page-write mark. HTTP clients cannot supply this request extension.
use provenance_store::{
    publication::with_publication_guard,
    repository::Repository,
    review_changes::{ReviewChanges, ReviewStateSnapshot},
};
use std::sync::Arc;

/// The owning host adds this mark after it authenticates the page's session cookie.
#[derive(Clone)]
pub struct ReviewPageWrite;

#[derive(Clone)]
pub struct PageWrites {
    pub repository: Repository,
    pub observed: Arc<dyn Fn(anyhow::Result<ReviewChanges>) + Send + Sync>,
}

impl PageWrites {
    pub fn run<R>(&self, work: impl FnOnce() -> R) -> anyhow::Result<R> {
        with_publication_guard(&self.repository, |guard| {
            let before = ReviewStateSnapshot::read(guard)?;
            let result = work();
            // Notification work cannot change the result of a completed write.
            (self.observed)(
                ReviewStateSnapshot::read(guard).map(|after| after.page_changes_since(&before)),
            );
            Ok(result)
        })
    }
}
