use provenance_macros::verifies;
use serde_json::Value;

use super::*;

const LANDINGS: RowIdentity = RowIdentity::Batch(BatchRecords::LANDINGS);

fn contribution(id: &str, finding: &str) -> Value {
    serde_json::json!({ "id": id, "strongest_finding": finding })
}

fn batch(list: &str, records: &[Value]) -> Value {
    serde_json::json!({ list: records })
}

#[test]
#[verifies("rule_record_merge", examples)]
fn a_record_that_the_batches_of_two_sides_change_differently_conflicts_and_keeps_ours() {
    let base = batch("contributions", &[contribution("c", "base")]);
    let ours = batch("contributions", &[contribution("c", "ours")]);
    let theirs = batch(
        "contributions",
        &[contribution("c", "theirs"), contribution("d", "theirs")],
    );
    let other = batch("contributions", &[contribution("e", "theirs")]);

    let MergeOutcome::Conflicted { conflicts, partial } = merge_records(
        LANDINGS,
        std::slice::from_ref(&base),
        &[base.clone(), ours.clone()],
        &[base.clone(), theirs, other.clone()],
    )
    .unwrap() else {
        panic!("expected a conflict");
    };

    assert_eq!(
        conflicts,
        [MergeConflict {
            kind: MergeConflictKind::DivergentEdit,
            record_id: "contributions/c".into(),
            base: Some(contribution("c", "base")),
            ours: Some(contribution("c", "ours")),
            theirs: Some(contribution("c", "theirs")),
        }]
    );
    assert_eq!(partial, [base, ours, other]);
}

#[test]
#[verifies("rule_record_merge", examples)]
fn a_proposal_that_the_batches_of_two_sides_add_differently_is_an_add_add_conflict() {
    let proposal = |title: &str| serde_json::json!({ "id": "p", "title": title });
    let ours = batch("proposals", &[proposal("ours")]);
    let theirs = batch("proposals", &[proposal("theirs")]);

    let conflicts = merge_records(LANDINGS, &[], &[ours], &[theirs])
        .unwrap()
        .unwrap_conflicts();

    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].kind, MergeConflictKind::AddAdd);
    assert_eq!(conflicts[0].record_id, "proposals/p");
    assert_eq!(conflicts[0].base, None);
}

#[test]
#[verifies("rule_record_merge", examples)]
fn a_record_that_the_batches_of_two_sides_hold_with_equal_content_merges_clean() {
    let ours = batch("contributions", &[contribution("c", "same"), contribution("o", "o")]);
    let theirs = batch("contributions", &[contribution("c", "same"), contribution("t", "t")]);

    let merged = merge_records(LANDINGS, &[], &[ours.clone()], &[theirs.clone()])
        .unwrap()
        .unwrap_clean();

    assert_eq!(merged, [ours, theirs]);
}

#[test]
#[verifies("rule_record_merge", examples)]
fn a_side_with_two_batches_of_one_record_is_compared_by_the_record_that_its_read_gives() {
    let first = batch("contributions", &[contribution("c", "first")]);
    let last = batch("contributions", &[contribution("c", "last")]);
    let theirs = batch(
        "contributions",
        &[contribution("c", "last"), contribution("t", "t")],
    );

    let merged = merge_records(LANDINGS, &[], &[first.clone(), last.clone()], &[theirs.clone()])
        .unwrap()
        .unwrap_clean();

    assert_eq!(merged, [first, last, theirs]);
}

// No Rule binding: an aid to the implementation. It shows the refusal of a
// batch row that a hand edit breaks.
#[test]
fn a_batch_list_that_is_not_a_list_is_refused() {
    let row = serde_json::json!({ "contributions": {} });

    let error = merge_records(LANDINGS, &[], &[row], &[]).unwrap_err();

    assert_eq!(error.to_string(), "batch field contributions is not a list");
}
