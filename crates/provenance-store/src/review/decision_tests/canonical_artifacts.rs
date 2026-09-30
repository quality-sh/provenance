use super::*;
use crate::write_error::{WriteError, WriteFailure};

#[test]
fn review_approval_rejects_a_canonical_artifact_for_another_record_kind() {
    for artifact_type in [
        "source",
        "resolution",
        "rule",
        "domain",
        "boundary",
        "topic",
        "question",
    ] {
        let (_temp, store, _, proposal) = enrolled();
        let error = decide(
            &store,
            &proposal,
            "accepted",
            &reviewer("reviewer"),
            &json!({
                "canonical_artifact": {
                    "artifact_type": artifact_type,
                    "artifact_id": "artifact_test"
                }
            }),
        )
        .unwrap_err();

        assert!(
            matches!(WriteError(error).safe(), WriteFailure::InvalidUpdate),
            "{artifact_type}"
        );
    }
}

#[test]
fn review_approval_rejects_a_canonical_artifact_for_another_record_id() {
    let (_temp, store, _, proposal) = enrolled();
    let error = decide(
        &store,
        &proposal,
        "accepted",
        &reviewer("reviewer"),
        &json!({
            "canonical_artifact": {
                "artifact_type": "requirement",
                "artifact_id": "req_b"
            }
        }),
    )
    .unwrap_err();

    assert!(matches!(
        WriteError(error).safe(),
        WriteFailure::InvalidUpdate
    ));
}
