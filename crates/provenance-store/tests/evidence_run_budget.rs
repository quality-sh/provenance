//! The evidence read of the public Rust API gives a verification run row
//! that the writer accepts: the list of runs is cut to the bytes that the
//! response budget of the native answer leaves, and its flag is set.

use crate::memory;
use crate::review_support::initialization;

use camino::Utf8Path;
use memory::begin_input;
use provenance_core::protocol::EvidenceQuery;
use provenance_core::{VerificationRun, VerificationRunStatus};
use provenance_macros::verifies;
use provenance_store::layout::ProvenanceLayout;
use provenance_store::operations::read_policy::ReadPolicy;
use provenance_store::operations::{begin_verification, complete_verification, queries};
use provenance_store::repository::Repository;
use provenance_store::state_store::CompleteVerificationInput;
use serde_json::json;
use std::path::Path;

/// The bytes of one run row that the reads of the runs give. The store owns
/// the number in `read_budget::resource_record_bytes`.
const BUDGET: usize = 1_048_576 - 16_384;

/// An initialized repository on the local disk with the active Rule
/// `rule_expiry` and the file that a run cites.
fn repository(root: &Path) -> Repository {
    std::fs::write(root.join("evidence.rs"), "fn expiry() {}\n").unwrap();
    initialization::initialize(
        &ProvenanceLayout::new(Utf8Path::from_path(root).unwrap()),
        &[],
    );
    let repository = Repository::open(root, "default").unwrap();
    let store = repository.store();
    let requirement = json!({
        "actor": "ben", "origin": null,
        "create": {
            "scope_id": "default", "id": "req_expiry", "statement": "Links expire",
            "status": "discovery", "depends_on": [], "supersedes": []
        }
    });
    store
        .create_review_requirement(serde_json::from_value(requirement).unwrap())
        .unwrap();
    let rule = json!({
        "scope_id": "default", "id": "rule_expiry", "requirement_ids": ["req_expiry"],
        "resolution_ids": [], "statement": "Links expire.", "status": "active",
        "severity": "high"
    });
    store
        .create_rule(serde_json::from_value(rule).unwrap())
        .unwrap();
    repository
}

/// A failed run of `rule_expiry` whose stored row is `row` bytes long.
fn failed_run(repository: &Repository, row: usize) -> VerificationRun {
    let run = begin_verification(repository, begin_input()).unwrap();
    let mut failed = run.clone();
    failed.status = VerificationRunStatus::Failed;
    failed.completed_at = Some(run.started_at);
    failed.error = Some(String::new());
    let error = "x".repeat(row - serde_json::to_vec(&failed).unwrap().len());
    let input = CompleteVerificationInput {
        run: run.id.as_str().to_owned(),
        status: "failed".to_owned(),
        error: Some(error),
    };
    complete_verification(repository, input).unwrap()
}

/// The evidence query of `rule_expiry` with a limit of 50 runs.
fn query() -> EvidenceQuery {
    EvidenceQuery {
        protocol_version: None,
        rule: "rule_expiry".to_owned(),
        base: None,
        head: None,
        limit: 50,
    }
}

#[tokio::test]
#[verifies("rule_query_pages_bound_shared_reads", examples)]
#[verifies("rule_evidence_flags_each_cut_list", examples)]
async fn the_native_evidence_read_cuts_its_runs_to_the_response_budget() {
    let root = tempfile::tempdir().unwrap();
    let repository = repository(root.path());
    let completed = failed_run(&repository, BUDGET);

    let answer = queries::evidence(&repository, ReadPolicy::default(), query())
        .await
        .unwrap()
        .result;

    assert_eq!(serde_json::to_vec(&completed).unwrap().len(), BUDGET);
    assert_eq!(answer.latest_verification_run, Some(completed));
    assert_eq!(answer.verification_runs, Vec::<VerificationRun>::new());
    assert!(answer.verification_runs_has_more);
    assert!(answer.has_more);
}

/// Two runs of 400 000 bytes and the newest run again as the latest run pass
/// the response budget: the oldest run leaves the list, and the newest stays.
#[tokio::test]
#[verifies("rule_query_pages_bound_shared_reads", examples)]
#[verifies("rule_evidence_flags_each_cut_list", examples)]
async fn the_native_evidence_read_keeps_the_newest_run_that_fits_and_leaves_out_the_oldest() {
    let root = tempfile::tempdir().unwrap();
    let repository = repository(root.path());
    let oldest = failed_run(&repository, 400_000);
    let newest = failed_run(&repository, 400_000);

    let answer = queries::evidence(&repository, ReadPolicy::default(), query())
        .await
        .unwrap()
        .result;

    assert!((oldest.started_at, oldest.id.as_str()) < (newest.started_at, newest.id.as_str()));
    assert_eq!(answer.verification_runs, vec![newest.clone()]);
    assert_eq!(answer.latest_verification_run, Some(newest));
    assert!(answer.verification_runs_has_more);
    assert!(answer.has_more);
}
