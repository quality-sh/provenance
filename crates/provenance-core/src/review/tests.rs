use super::ReviewRecord;
use crate::NodeType;
use serde_json::json;

/// Implementation aid: pins the closed read of an enrolled record; no Rule
/// names it.
#[test]
fn closed_record_read_refuses_unknown_fields() {
    let mut value = json!({
        "scope_id": "default",
        "id": "source_a",
        "name": "Policy",
        "source_type": "document",
        "url": null
    });
    let record = ReviewRecord::deserialize_closed(NodeType::Source, &value).unwrap();
    assert!(matches!(record, ReviewRecord::Source(_)));
    value["unexpected"] = json!(true);
    let error = ReviewRecord::deserialize_closed(NodeType::Source, &value)
        .unwrap_err()
        .to_string();
    assert!(error.contains("unknown field `unexpected`"), "{error}");
}

mod stored_rows {
    use crate::review::{read_review_rows, ReviewRowFamily, ReviewRowFault, RowAt, StoredReviewRows};
    use crate::{DispositionDecision, DispositionRecord, NodeType, ProposalCard, StableId, Withdrawal};
    use provenance_macros::verifies;
    use serde_json::json;

    fn id(value: &str) -> StableId {
        StableId::new(value).unwrap()
    }

    fn submission(id: &str, record: &str, cycle: u64) -> ProposalCard {
        serde_json::from_value(json!({
            "scope_id": "default", "id": id,
            "proposal_key": format!("review:requirement:{record}:{cycle}"),
            "proposal_type": "record_revision", "title": "Review", "summary": "Review",
            "traceability": {
                "target": {"artifact_type": "requirement", "artifact_id": record},
                "source_ids": [], "evidence_references": [], "supporting_claim_ids": []
            },
            "promotion_state": "proposed",
            "record_revision": {"revision": "rev_a"}
        }))
        .unwrap()
    }

    fn decision(id: &str, submission: &str, result: &str, actor: &str) -> DispositionRecord {
        serde_json::from_value(json!({
            "scope_id": "default", "id": id, "proposal_id": submission, "decision": result,
            "actor": {"identity_type": "human", "id": actor}
        }))
        .unwrap()
    }

    fn withdrawal(id: &str, submission: &str, actor: &str) -> Withdrawal {
        serde_json::from_value(json!({
            "scope_id": "default", "id": id, "proposal_id": submission, "actor": actor,
            "reason": null
        }))
        .unwrap()
    }

    fn at(family: ReviewRowFamily, position: usize) -> RowAt {
        RowAt { family, position }
    }

    const REQ_A: &str = "req_a";

    /// Pairs 3 to 7: two decisions with one result, from other actors, read
    /// as one decision with that result.
    #[test]
    #[verifies("rule_same_result_decisions_read_as_one", examples)]
    #[verifies("rule_review_history_lists_each_row_once", examples)]
    fn decisions_with_one_result_read_as_that_result() {
        let proposals = [submission("sub_a", REQ_A, 1)];
        for result in ["accepted", "rejected", "deferred"] {
            let dispositions = [
                decision("dec_b", "sub_a", result, "reviewer"),
                decision("dec_a", "sub_a", result, "lead"),
            ];
            let reading = read_review_rows(StoredReviewRows {
                proposals: &proposals,
                dispositions: &dispositions,
                withdrawals: &[],
            });
            let expected: DispositionDecision = serde_json::from_value(json!(result)).unwrap();
            assert_eq!(reading.agreed_decision(&id("sub_a")), Some(expected));
            let record = reading.record(NodeType::Requirement, &id(REQ_A)).unwrap();
            let history = record
                .decisions()
                .into_iter()
                .map(|(row, _)| row.id.as_str())
                .collect::<Vec<_>>();
            assert_eq!(history, ["dec_a", "dec_b"], "sorted by the row ID");
            assert!(record.is_decided(&id("sub_a")));
            assert_eq!(record.pending_at(Some(&id("rev_a"))), None);
            assert_eq!(reading.faults().count(), 0);
        }
    }

    /// Pairs 2, 17 and 18: one ID on two equal lines reads as one row.
    #[test]
    #[verifies("rule_equal_review_row_lines_read_as_one", examples)]
    #[verifies("rule_repeated_withdrawals_read_as_one", examples)]
    fn equal_lines_of_one_id_read_as_one_row() {
        let proposals = [submission("sub_a", REQ_A, 1), submission("sub_a", REQ_A, 1)];
        let dispositions = [
            decision("dec_a", "sub_a", "rejected", "reviewer"),
            decision("dec_a", "sub_a", "rejected", "reviewer"),
        ];
        let withdrawals = [
            withdrawal("wd_a", "sub_a", "agent"),
            withdrawal("wd_a", "sub_a", "agent"),
            withdrawal("wd_b", "sub_a", "editor"),
        ];
        let reading = read_review_rows(StoredReviewRows {
            proposals: &proposals,
            dispositions: &dispositions,
            withdrawals: &withdrawals,
        });
        let once = reading.rows_once();
        assert_eq!(
            (&once.proposals, &once.dispositions, &once.withdrawals),
            (&vec![0], &vec![0], &vec![0, 2])
        );
        let record = reading.record(NodeType::Requirement, &id(REQ_A)).unwrap();
        assert_eq!(record.submissions().count(), 1);
        assert_eq!(record.decisions().len(), 1);
        assert_eq!(record.withdrawn(), [id("sub_a")]);
        assert_eq!(reading.scope_fault(), None);
    }

    /// Each case of review rows that do not agree refuses the record that
    /// the rows name, and a different record reads as usual.
    #[test]
    #[verifies("rule_disagreeing_review_rows_refuse_record", examples)]
    fn rows_that_disagree_refuse_only_their_record() {
        let decision_family = ReviewRowFamily::Decision;
        let proposals = [submission("sub_a", REQ_A, 1), submission("sub_b", "req_b", 1)];
        let cases: Vec<(Vec<DispositionRecord>, Vec<Withdrawal>, ReviewRowFault)> = vec![
            (
                vec![
                    decision("dec_a", "sub_a", "accepted", "reviewer"),
                    decision("dec_b", "sub_a", "rejected", "reviewer"),
                ],
                vec![],
                ReviewRowFault::DifferentResults {
                    submission: id("sub_a"),
                    first: at(decision_family, 0),
                    second: at(decision_family, 1),
                },
            ),
            (
                vec![decision("dec_a", "sub_a", "accepted", "reviewer")],
                vec![withdrawal("wd_a", "sub_a", "agent"), withdrawal("wd_b", "sub_a", "agent")],
                ReviewRowFault::AcceptedAndWithdrawn {
                    submission: id("sub_a"),
                    approval: at(decision_family, 0),
                    withdrawal: at(ReviewRowFamily::Withdrawal, 0),
                },
            ),
            (
                vec![
                    decision("dec_a", "sub_a", "deferred", "reviewer"),
                    decision("dec_a", "sub_a", "deferred", "lead"),
                ],
                vec![],
                ReviewRowFault::IdOnTwoLines {
                    id: id("dec_a"),
                    first: at(decision_family, 0),
                    second: at(decision_family, 1),
                },
            ),
        ];
        for (dispositions, withdrawals, expected) in cases {
            let reading = read_review_rows(StoredReviewRows {
                proposals: &proposals,
                dispositions: &dispositions,
                withdrawals: &withdrawals,
            });
            assert_eq!(
                reading.record(NodeType::Requirement, &id(REQ_A)).unwrap_err(),
                &expected
            );
            assert!(reading.record(NodeType::Requirement, &id("req_b")).is_ok());
            assert_eq!(reading.faults().collect::<Vec<_>>(), [&expected]);
            assert_eq!(reading.scope_fault(), None);
        }
    }

    /// Pairs 14 and 18: two submissions of one record with one cycle, and a
    /// submission ID on two lines that disagree.
    #[test]
    #[verifies("rule_disagreeing_review_rows_refuse_record", examples)]
    fn submissions_that_disagree_refuse_their_record() {
        let family = ReviewRowFamily::Submission;
        let one_cycle = [submission("sub_b", REQ_A, 2), submission("sub_a", REQ_A, 2)];
        let reading = read_review_rows(StoredReviewRows {
            proposals: &one_cycle,
            dispositions: &[],
            withdrawals: &[],
        });
        assert_eq!(
            reading.record(NodeType::Requirement, &id(REQ_A)).unwrap_err(),
            &ReviewRowFault::OneCycleTwoSubmissions {
                cycle: 2,
                first: at(family, 0),
                second: at(family, 1),
            }
        );
        let two_lines = [submission("sub_a", REQ_A, 1), submission("sub_a", REQ_A, 2)];
        let reading = read_review_rows(StoredReviewRows {
            proposals: &two_lines,
            dispositions: &[],
            withdrawals: &[],
        });
        assert_eq!(reading.rows_once().proposals, Vec::<usize>::new());
        assert!(matches!(
            reading.record(NodeType::Requirement, &id(REQ_A)),
            Err(ReviewRowFault::IdOnTwoLines { .. })
        ));
    }

    /// A rejection or a deferral beside a withdrawal is a correct state, and
    /// a withdrawal that names no submission is a fault of the scope.
    #[test]
    #[verifies("rule_disagreeing_review_rows_refuse_record", examples)]
    fn correct_states_read_and_a_withdrawal_with_no_submission_is_a_scope_fault() {
        let proposals = [submission("sub_a", REQ_A, 1)];
        for result in ["rejected", "deferred"] {
            let dispositions = [decision("dec_a", "sub_a", result, "reviewer")];
            let withdrawals = [withdrawal("wd_a", "sub_a", "agent")];
            let reading = read_review_rows(StoredReviewRows {
                proposals: &proposals,
                dispositions: &dispositions,
                withdrawals: &withdrawals,
            });
            assert!(reading.record(NodeType::Requirement, &id(REQ_A)).is_ok());
        }
        let withdrawals = [withdrawal("wd_a", "sub_none", "agent")];
        let reading = read_review_rows(StoredReviewRows {
            proposals: &proposals,
            dispositions: &[],
            withdrawals: &withdrawals,
        });
        assert_eq!(
            reading.scope_fault(),
            Some(&ReviewRowFault::NoSubmission {
                withdrawal: at(ReviewRowFamily::Withdrawal, 0),
                proposal: id("sub_none"),
            })
        );
        assert!(reading.record(NodeType::Requirement, &id(REQ_A)).is_ok());
    }
}
