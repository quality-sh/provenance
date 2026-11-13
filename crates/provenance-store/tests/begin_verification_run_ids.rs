//! The run ids that starts of verification runs take at one clock value, over
//! the memory adapter and a fixed clock. No Rule states the form or the
//! uniqueness of a run id, so these tests have no Rule binding.

#[path = "support/memory.rs"]
mod memory;

use memory::{begin_input, scope, verification_repository, FixedClock, Recorded};
use provenance_store::operations::begin_verification;
use provenance_store::repository::Repository;
use std::sync::Arc;

const NOW: i64 = 1_767_225_600_000;

fn repository(root: &tempfile::TempDir) -> Repository {
    let clock = Arc::new(FixedClock::at_millis(NOW));
    verification_repository(root.path(), [], Arc::new(Recorded::default()), clock).1
}

/// The ids of the rows of the runs cache.
fn row_ids(repository: &Repository) -> Vec<String> {
    let runs = repository.store().list_verification_runs(&scope()).unwrap();
    runs.into_iter().map(|run| run.id.as_str().to_owned()).collect()
}

#[test]
fn two_starts_at_one_clock_value_get_different_run_ids_and_both_rows_land() {
    let root = tempfile::tempdir().unwrap();
    let repository = repository(&root);

    let first = begin_verification(&repository, begin_input()).unwrap();
    let second = begin_verification(&repository, begin_input()).unwrap();

    let ids = [format!("verification_{NOW}"), format!("verification_{NOW}_2")];
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

    let reviews = repository.store().list_requirement_reviews(&scope()).unwrap();
    assert_eq!(reviews[0].cleared_by_run.as_ref(), Some(&first.id));
    assert_eq!(second.id.as_str(), format!("verification_{NOW}_2"));
    assert_eq!(row_ids(&repository), [second.id.as_str()]);
}
