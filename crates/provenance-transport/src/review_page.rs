//! A trusted page-write mark. HTTP clients cannot supply this request extension.
use provenance_macros::rule;
use provenance_store::{
    publication::with_publication_guard,
    repository::Repository,
    review_changes::{ReviewChanges, ReviewStateSnapshot},
};
use std::sync::Arc;

/// The owning host adds this mark after it authenticates the page session.
#[derive(Clone)]
pub struct ReviewPageWrite;

#[derive(Clone)]
pub struct PageWrites {
    pub repository: Repository,
    pub observed: Arc<dyn Fn(anyhow::Result<ReviewChanges>) + Send + Sync>,
}

impl PageWrites {
    /// Runs `work` in one publication section with the repository of that
    /// section, and reads the changes of the page write before and after it.
    /// The observer gets the changes, or the error when a read fails. A failed
    /// read does not change the result. When the section does not start, the
    /// observer gets that error and `work` runs on the host repository.
    #[rule("rule_page_write_runs_in_its_change_section")]
    #[rule("rule_page_write_keeps_result_when_capture_fails")]
    pub fn run<R>(&self, work: impl FnOnce(&Repository) -> R) -> R {
        let mut work = Some(work);
        let mut result = None;
        let section = with_publication_guard(&self.repository, |guard| {
            let before = ReviewStateSnapshot::read(guard);
            let run = work.take().expect("the section runs its work once");
            result = Some(run(guard.repository()));
            Ok(before.and_then(|before| {
                ReviewStateSnapshot::read(guard).map(|after| after.page_changes_since(&before))
            }))
        });
        // Notification work cannot change the result of a completed write.
        (self.observed)(section.and_then(|changes| changes));
        match (result, work) {
            (Some(result), _) => result,
            (None, Some(work)) => work(&self.repository),
            (None, None) => unreachable!("the work ran and returned"),
        }
    }
}

#[cfg(all(test, feature = "test-fixture"))]
mod tests {
    use super::PageWrites;
    use crate::fixture::records::Repository;
    use provenance_macros::verifies;
    use std::sync::{Arc, Mutex};

    #[test]
    #[verifies("rule_page_write_keeps_result_when_capture_fails", examples)]
    fn a_page_write_keeps_its_result_when_the_after_read_fails() {
        let repo = Repository::new("The graph is readable.");
        let notifications = Arc::new(Mutex::new(Vec::new()));
        let seen = notifications.clone();
        let page = PageWrites {
            repository: repo.open(),
            observed: Arc::new(move |changes| seen.lock().unwrap().push(changes)),
        };
        let settings = repo.dir.path().join(".provenance/settings.json");
        let result = page.run(|held| {
            assert_eq!(held.working_tree(), repo.open().working_tree());
            std::fs::write(&settings, "invalid read settings").unwrap();
            "the write result"
        });
        assert_eq!(result, "the write result");
        let notifications = std::mem::take(&mut *notifications.lock().unwrap());
        assert_eq!(notifications.len(), 1);
        assert!(notifications[0].is_err());
    }
}
