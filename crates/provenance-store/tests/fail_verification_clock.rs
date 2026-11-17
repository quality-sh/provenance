//! A failed completion cuts its failure text from the stored row of the run,
//! with the completion time that the store reads as it completes the run.

use crate::memory;

use memory::{begin_input, verification_repository, Recorded};
use provenance_core::{VerificationRun, VerificationRunStatus};
use provenance_macros::verifies;
use provenance_store::operations::{begin_verification, fail_verification, verification_runs};
use provenance_store::repository::Clock;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

/// The bytes of one run row that the reads of the runs give. The store owns
/// the number in `read_budget::resource_record_bytes`.
const BUDGET: usize = 1_048_576 - 16_384;

/// A clock that reads the milliseconds after the Unix epoch that the test
/// sets.
#[derive(Debug)]
struct SetClock(AtomicI64);

impl Clock for SetClock {
    fn now(&self) -> time::OffsetDateTime {
        let nanos = i128::from(self.0.load(Ordering::SeqCst)) * 1_000_000;
        time::OffsetDateTime::from_unix_timestamp_nanos(nanos).unwrap()
    }
}

/// The bytes of the failed row of `run` with an empty text and a completion
/// time equal to its start time.
fn row_without_text(run: &VerificationRun) -> usize {
    let mut failed = run.clone();
    failed.status = VerificationRunStatus::Failed;
    failed.completed_at = Some(run.started_at);
    failed.error = Some(String::new());
    serde_json::to_vec(&failed).unwrap().len()
}

#[test]
#[verifies("rule_rust_verification_cuts_a_long_failure_text", examples)]
fn a_completion_time_one_digit_longer_than_the_start_time_gives_a_row_inside_the_budget() {
    let root = tempfile::tempdir().unwrap();
    let clock = Arc::new(SetClock(AtomicI64::new(9_999)));
    let persistence = Arc::new(Recorded::default());
    let (_, repository) = verification_repository(root.path(), &[], [], persistence, clock.clone());
    let run = begin_verification(&repository, begin_input()).unwrap();
    clock.0.store(10_000, Ordering::SeqCst);
    let text = "x".repeat(BUDGET + 1 - row_without_text(&run));

    let failed = fail_verification(&repository, run.id.as_str(), &text).unwrap();

    assert_eq!(
        (failed.status, failed.started_at, failed.completed_at),
        (VerificationRunStatus::Failed, 9_999, Some(10_000))
    );
    assert_eq!(serde_json::to_vec(&failed).unwrap().len(), BUDGET);
    let stored = failed.error.as_deref().unwrap();
    let kept = stored.rfind("… [cut: ").unwrap();
    assert_eq!(&stored[..kept], &text[..kept]);
    assert_eq!(
        &stored[kept..],
        format!("… [cut: {kept} of {} bytes]", text.len())
    );
    assert_eq!(verification_runs(&repository, None).unwrap(), vec![failed]);
}
