use super::*;
use crate::write_error::{WriteError, WriteFailure};

#[test]
fn requirement_review_rejects_new_canonical_artifact_kinds_as_invalid_updates() {
    for artifact_type in ["domain", "boundary", "topic", "question"] {
        let (_temp, store, _) = enrolled();
        let error = decide(
            &store,
            "decide-1",
            "prop-1",
            "disp-1",
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
