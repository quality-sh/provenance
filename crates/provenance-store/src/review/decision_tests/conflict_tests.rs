use super::*;
use crate::write_error::WriteError;

#[test]
fn repeated_decision_currently_becomes_an_internal_write_failure() {
    let (_temp, store, _) = enrolled();
    decide(
        &store,
        "decide-1",
        "prop-1",
        "disp-1",
        "rejected",
        &reviewer("reviewer"),
        &json!({}),
    )
    .unwrap();

    let error = decide(
        &store,
        "decide-2",
        "prop-1",
        "disp-2",
        "accepted",
        &reviewer("reviewer"),
        &artifact(),
    )
    .unwrap_err();

    assert_eq!(WriteError(error).status(), 500);
}
