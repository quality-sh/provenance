//! The run ids that starts of verification runs take at one clock value, over
//! the memory adapter and a fixed clock. No Rule states the form or the
//! uniqueness of a run id, so these tests have no Rule binding.

#[path = "support/memory.rs"]
mod memory;

use memory::{begin_input, open_review, scope, verification_repository, FixedClock, Recorded};
use provenance_store::operations::begin_verification;
use provenance_store::repository::Repository;
use provenance_store::storage::{PublishFailure, StatePersistence, WriteSet};
use provenance_store::write_error::WriteError;
use serde_json::json;
use std::sync::{Arc, Mutex};

const NOW: i64 = 1_767_225_600_000;

fn repository(root: &tempfile::TempDir) -> Repository {
    repository_over(root, Arc::new(Recorded::default()))
}

/// A repository at `root` whose adapter gives each write set to
/// `persistence`.
fn repository_over(
    root: &tempfile::TempDir,
    persistence: Arc<dyn StatePersistence>,
) -> Repository {
    let clock = Arc::new(FixedClock::at_millis(NOW));
    let reviews = [open_review()];
    verification_repository(root.path(), &reviews, [], persistence, clock).1
}

/// Keeps each write set. While it keeps the first set, it starts a run in
/// `other`, a second repository over the same working tree with its own
/// section.
#[derive(Debug)]
struct StartsAnotherRun {
    other: Mutex<Option<Repository>>,
}

impl StatePersistence for StartsAnotherRun {
    fn persist(&self, _writes: &WriteSet) -> Result<(), PublishFailure> {
        if let Some(other) = self.other.lock().unwrap().take() {
            begin_verification(&other, begin_input()).unwrap();
        }
        Ok(())
    }
}

/// The ids of the rows of the runs cache.
fn row_ids(repository: &Repository) -> Vec<String> {
    let runs = repository.store().list_verification_runs(&scope()).unwrap();
    runs.into_iter()
        .map(|run| run.id.as_str().to_owned())
        .collect()
}

#[test]
fn two_starts_at_one_clock_value_get_different_run_ids_and_both_rows_land() {
    let root = tempfile::tempdir().unwrap();
    let repository = repository(&root);

    let first = begin_verification(&repository, begin_input()).unwrap();
    let second = begin_verification(&repository, begin_input()).unwrap();

    let ids = [
        format!("verification_{NOW}"),
        format!("verification_{NOW}_2"),
    ];
    assert_eq!([first.id.as_str(), second.id.as_str()], ids);
    assert_eq!(row_ids(&repository), ids);
}

/// The runs cache is derived state that a user can delete. A cleared review
/// still names the run that cleared it.
#[test]
fn a_start_after_the_runs_cache_is_deleted_takes_an_id_that_no_review_names() {
    let root = tempfile::tempdir().unwrap();
    let repository = repository(&root);
    let first = begin_verification(&repository, begin_input()).unwrap();
    let runs = ".provenance/cache/scopes/default/verification-runs.jsonl";
    std::fs::remove_file(root.path().join(runs)).unwrap();

    let second = begin_verification(&repository, begin_input()).unwrap();

    let reviews = repository
        .store()
        .list_requirement_reviews(&scope())
        .unwrap();
    assert_eq!(reviews[0].cleared_by_run.as_ref(), Some(&first.id));
    assert_eq!(second.id.as_str(), format!("verification_{NOW}_2"));
    assert_eq!(row_ids(&repository), [second.id.as_str()]);
}

/// Two repositories over the memory adapter and one working tree have
/// separate sections, so a start in each can take one run id.
#[test]
fn a_start_whose_run_id_a_row_holds_adds_no_row() {
    let root = tempfile::tempdir().unwrap();
    let other = repository(&root);
    let persistence = Arc::new(StartsAnotherRun {
        other: Mutex::new(Some(other)),
    });
    let repository = repository_over(&root, persistence);

    let error = WriteError(begin_verification(&repository, begin_input()).unwrap_err());

    let cause = format!("the runs cache already holds verification run `verification_{NOW}`");
    assert_eq!(
        (serde_json::to_value(&error).unwrap(), error.status()),
        (json!({"kind": "write_failed", "cause": cause}), 500)
    );
    assert_eq!(row_ids(&repository), [format!("verification_{NOW}")]);
}
