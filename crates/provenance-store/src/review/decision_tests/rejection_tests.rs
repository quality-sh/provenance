use super::*;

#[test]
#[provenance_macros::verifies("rule_rejection_keeps_graph_record", examples)]
fn rejection_does_not_remove_or_retire_the_graph_record() {
    let (_temp, store, _, proposal) = enrolled();
    let before = store.list_requirements(&scope()).unwrap().remove(0);

    decide(
        &store,
        &proposal,
        "rejected",
        &reviewer("reviewer"),
        &json!({}),
    )
    .unwrap();

    let requirement = store.list_requirements(&scope()).unwrap().remove(0);
    assert_eq!(requirement.id, req(), "rejection must keep Requirement req_a");
    assert_eq!(
        requirement.status, before.status,
        "rejection must not retire the Requirement graph record"
    );
}
