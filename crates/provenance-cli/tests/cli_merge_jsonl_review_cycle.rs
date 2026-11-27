//! The merge of the review submissions in the Proposal file of a scope.
//!
//! Two branches from one base give their review submissions of one record the
//! same review cycle, and each read of a record with two submissions of one
//! cycle refuses. So the merge reports a conflict, and git leaves the file for
//! a person.

use super::cli_merge_jsonl::Sides;
use provenance_core::protocol::read_failure::STATE_CORRECTIONS;
use provenance_macros::verifies;
use serde_json::{json, Value};

const PROPOSALS_SHARD: &str = ".provenance/state/scopes/default/ideation/proposal_cards.jsonl";

/// A review submission of `requirement` with review cycle `cycle`, as the
/// store writes it.
fn submission_in_cycle(id: &str, requirement: &str, cycle: u64, summary: &str) -> String {
    format!(
        "{{\"scope_id\":\"default\",\"id\":\"{id}\",\
         \"proposal_key\":\"review:requirement:{requirement}:{cycle}\",\
         \"proposal_type\":\"record_revision\",\"title\":\"Review {requirement}\",\
         \"summary\":\"{summary}\",\"traceability\":{{\"target\":\
         {{\"artifact_type\":\"requirement\",\"artifact_id\":\"{requirement}\"}},\
         \"source_ids\":[],\"evidence_references\":[],\"supporting_claim_ids\":[]}},\
         \"promotion_state\":\"proposed\",\"record_revision\":{{\"revision\":\
         \"{}\"}},\"actor\":\"cli\"}}\n",
        "a".repeat(64)
    )
}

/// A review submission of `requirement` with review cycle 1.
fn submission(id: &str, requirement: &str) -> String {
    submission_in_cycle(id, requirement, 1, &format!("The change of {id}."))
}

fn value(line: &str) -> Value {
    serde_json::from_str(line).unwrap()
}

/// The JSON report of a merge that exits 1.
fn conflict_report(sides: &Sides, shard_path: &str) -> Value {
    let failure = sides.merge(shard_path).assert().code(1);
    serde_json::from_slice(&failure.get_output().stdout).unwrap()
}

/// Two branches each submit a review of `req_a` from one base, so each
/// submission has review cycle 1. The merge reports the submission of their
/// branch, and the output file keeps both submissions, so that the read of
/// `req_a` after the person accepts the file names each line to remove.
#[test]
#[verifies("rule_merge_reports_same_review_cycle", examples)]
fn two_submissions_of_one_record_with_one_cycle_report_a_conflict() {
    let directory = tempfile::tempdir().unwrap();
    let ours = submission("submission_ours", "req_a");
    let theirs = submission("submission_theirs", "req_a");
    let sides = Sides::write(directory.path(), "", &ours, &theirs);

    let report = conflict_report(&sides, PROPOSALS_SHARD);

    assert_eq!(
        report,
        json!({"status":"conflicted", "conflicts":[{"kind":"same_review_cycle",
            "record_id":"submission_theirs", "base":null, "ours":value(&ours),
            "theirs":value(&theirs)}], "partial":[value(&ours), value(&theirs)]})
    );
    assert_eq!(
        std::fs::read_to_string(&sides.output).unwrap(),
        format!("{ours}{theirs}")
    );
}

/// Each branch submits a review of a different record. The merge keeps
/// both submissions, as for each other record that only one side adds.
#[test]
#[verifies("rule_record_merge", examples)]
fn submissions_of_two_records_merge_to_both_rows() {
    let directory = tempfile::tempdir().unwrap();
    let ours = submission("submission_ours", "req_a");
    let theirs = submission("submission_theirs", "req_b");
    let sides = Sides::write(directory.path(), "", &ours, &theirs);

    sides.merge(PROPOSALS_SHARD).assert().success();

    assert_eq!(
        std::fs::read_to_string(&sides.output).unwrap(),
        format!("{ours}{theirs}")
    );
}

/// Each branch adds the same submission line. The merge by ID reads the two
/// lines as one row, so no two submissions share a cycle.
#[test]
fn the_same_submission_on_both_sides_merges_to_one_row() {
    let directory = tempfile::tempdir().unwrap();
    let both = submission("submission_both", "req_a");
    let sides = Sides::write(directory.path(), "", &both, &both);

    sides.merge(PROPOSALS_SHARD).assert().success();

    assert_eq!(std::fs::read_to_string(&sides.output).unwrap(), both);
}

/// Each branch submits a review of `req_a`, but with different review
/// cycles. The read of `req_a` accepts the two submissions, so the merge
/// keeps both.
#[test]
fn submissions_of_one_record_with_two_cycles_merge_to_both_rows() {
    let directory = tempfile::tempdir().unwrap();
    let ours = submission_in_cycle("submission_ours", "req_a", 1, "First.");
    let theirs = submission_in_cycle("submission_theirs", "req_a", 2, "Second.");
    let sides = Sides::write(directory.path(), &ours, &ours, &format!("{ours}{theirs}"));

    sides.merge(PROPOSALS_SHARD).assert().success();

    assert_eq!(
        std::fs::read_to_string(&sides.output).unwrap(),
        format!("{ours}{theirs}")
    );
}

/// Only the Proposal file of a scope holds review submissions. The same
/// rows at another path merge by ID with no check of the review cycles.
#[test]
fn submissions_with_one_cycle_at_another_path_merge_to_both_rows() {
    let directory = tempfile::tempdir().unwrap();
    let ours = submission("submission_ours", "req_a");
    let theirs = submission("submission_theirs", "req_a");
    let sides = Sides::write(directory.path(), "", &ours, &theirs);

    for path in [
        ".provenance/state/scopes/default/ideation/dispositions.jsonl",
        "fixtures/ideation/proposal_cards.jsonl",
    ] {
        sides.merge(path).assert().success();

        assert_eq!(
            std::fs::read_to_string(&sides.output).unwrap(),
            format!("{ours}{theirs}"),
            "{path}"
        );
    }
}

/// Our branch removes the submission S of the base and submits again with
/// the cycle of S. Their branch changes S. The merge by ID reports S as
/// `delete_modify` and keeps their S, and S shares the cycle of our new
/// submission. The report names S one time, and the file holds each
/// submission that the report names.
#[test]
#[verifies("rule_merge_reports_same_review_cycle", examples)]
fn a_submission_that_one_side_removes_and_the_other_changes_is_reported_one_time() {
    let directory = tempfile::tempdir().unwrap();
    let base = submission_in_cycle("submission_base", "req_a", 1, "Base.");
    let changed = submission_in_cycle("submission_base", "req_a", 1, "Changed.");
    let ours = submission_in_cycle("submission_new", "req_a", 1, "Again.");
    let sides = Sides::write(directory.path(), &base, &ours, &changed);

    let report = conflict_report(&sides, PROPOSALS_SHARD);

    assert_eq!(
        report,
        json!({"status":"conflicted", "conflicts":[{"kind":"delete_modify",
            "record_id":"submission_base", "base":value(&base), "ours":null,
            "theirs":value(&changed)}], "partial":[value(&changed), value(&ours)]})
    );
    assert_eq!(
        std::fs::read_to_string(&sides.output).unwrap(),
        format!("{changed}{ours}")
    );
}

/// A merged line of the Proposal file that is not a Proposal refuses the
/// merge at the line of its side, as a read of the file refuses it. The
/// output file stays as it was.
#[test]
#[verifies("rule_state_parse_refusal_names_the_line", examples)]
fn a_merged_line_that_is_not_a_proposal_refuses_at_its_line() {
    let directory = tempfile::tempdir().unwrap();
    let ours = submission("submission_ours", "req_a");
    let theirs = format!("{ours}{{\"scope_id\":\"default\",\"id\":\"proposal_x\"}}\n");
    let sides = Sides::write(directory.path(), &ours, &ours, &theirs);
    std::fs::write(&sides.output, "unchanged\n").unwrap();

    let output = sides.merge(PROPOSALS_SHARD).output().unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "");
    let error = json!({"error":{"file":"scopes/default/ideation/proposal_cards.jsonl",
        "hint":STATE_CORRECTIONS, "kind":"invalid_state_file", "line":2,
        "message":"theirs: missing field `proposal_key`"}, "meta":{}});
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        format!("{error}\n")
    );
    assert_eq!(
        std::fs::read_to_string(&sides.output).unwrap(),
        "unchanged\n"
    );
}
