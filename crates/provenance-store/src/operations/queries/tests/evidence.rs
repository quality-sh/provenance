//! `evidence` reads its bindings and reviews from the projection, says
//! which of its four lists a limit cut, and answers only the reviews
//! still open.

use super::comparison::requests;
use super::comparison::test_stores::TestStore;
use crate::operations::queries;
use provenance_macros::verifies;

/// `rule_overtime_001` has two active implementation bindings and a
/// retired third, three verification bindings, two open reviews, and no
/// run. A limit of two cuts the verification list alone; asked for
/// retired bindings, the implementation list is cut too.
#[tokio::test]
#[verifies("rule_evidence_flags_each_cut_list", examples)]
async fn evidence_reports_which_list_was_cut() {
    let store = TestStore::pinned();
    let mut request = requests::evidence("rule_overtime_001", None);
    request.limit = 2;
    let answer = queries::evidence(Some(store.root.clone()), &store.scope, request.clone())
        .await
        .unwrap()
        .result;
    assert_eq!(answer.implementation_bindings.len(), 2);
    assert!(!answer.implementation_bindings_has_more);
    assert_eq!(answer.verification_bindings.len(), 2);
    assert!(answer.verification_bindings_has_more);
    assert!(answer.verification_runs.is_empty());
    assert!(!answer.verification_runs_has_more);
    assert_eq!(answer.reviews.len(), 2);
    assert!(!answer.reviews_has_more);
    assert!(answer.has_more, "the top-level flag is the OR of the four");

    request.include_retired = true;
    let with_retired = queries::evidence(Some(store.root.clone()), &store.scope, request)
        .await
        .unwrap()
        .result;
    assert!(with_retired.implementation_bindings_has_more);
    assert_eq!(with_retired.implementation_bindings.len(), 2);
}

/// `review_b` was cleared by a run; it is neither answered nor counted
/// toward `review_required`, and the review table stands behind the
/// answer.
#[tokio::test]
async fn a_cleared_review_is_not_open() {
    let store = TestStore::pinned();
    let answer = queries::evidence(
        Some(store.root.clone()),
        &store.scope,
        requests::evidence("rule_overtime_001", None),
    )
    .await
    .unwrap();
    let reviews: Vec<&str> = answer
        .result
        .reviews
        .iter()
        .map(|review| review.id.as_str())
        .collect();
    assert_eq!(reviews, ["review_a", "review_c"]);
    assert!(answer.result.review_required);
    assert_eq!(
        answer.stamp.attested,
        [
            "implementation_bindings",
            "requirement_reviews",
            "verification_bindings"
        ]
    );
    assert_eq!(answer.stamp.live, ["verification_runs"]);
}
