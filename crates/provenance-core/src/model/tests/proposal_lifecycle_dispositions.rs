#[test]
#[verifies("rule_qualified_proposal_assertion", examples)]
fn rejected_disposition_allows_blocked_or_contested_proposal_without_assertion() {
    for blocked in ["evidence_gap", "contested_claim"] {
        let (contribution, mut synthesis, proposal, _) = lifecycle_fixture();
        if blocked == "evidence_gap" {
            synthesis["evidence_gaps"] = serde_json::json!([{
                "question": "Unknown", "needed_evidence_type": "source", "blocking_promotion": true
            }]);
        } else {
            synthesis["contested_claims"] = serde_json::json!([{
                "claim_id": "claim_a", "statement": "Disputed", "supporting_participant_slots": [],
                "opposing_participant_slots": ["refuter"], "evidence_quality": "weak"
            }]);
        }
        let contributions = vec![serde_json::from_value(contribution).unwrap()];
        let synthesis_packets = vec![serde_json::from_value(synthesis).unwrap()];
        let proposals = vec![serde_json::from_value(proposal).unwrap()];
        let dispositions = vec![serde_json::from_value(serde_json::json!({
            "schema_version": 1, "scope_id": "default", "id": format!("disposition_{blocked}"),
            "proposal_id": "proposal_a", "decision": "rejected", "rationale": "Did not pass adjudication",
            "actor": {"identity_type": "human", "id": "reviewer"}
        })).unwrap()];

        crate::validate_ideation_aggregate(crate::IdeationAggregate {
            legacy_policy: crate::LegacyProposalPolicy::ModernOnly,
            ratification: crate::DispositionRatification::LegacyAllowlist(&["reviewer".into()]),
            contributions: &contributions,
            synthesis_packets: &synthesis_packets,
            proposals: &proposals,
            assertions: &[],
            dispositions: &dispositions,
        })
        .unwrap();
    }
}

/// The end of a fork tournament: the packet still holds the human decision
/// that blocks promotion, so no swarm ever asserted the winner. The person
/// picks it, lands the choice as a ratified artifact, and the disposition
/// names that artifact.
fn validate_ratified_acceptance(identity_type: &str, names_artifact: bool) -> anyhow::Result<()> {
    let (contribution, mut synthesis, proposal, _) = lifecycle_fixture();
    synthesis["required_human_decisions"] = serde_json::json!([{
        "decision_key": "pick_winner", "prompt": "Pick the winning fork", "blocks_promotion": true
    }]);
    let contributions = vec![serde_json::from_value(contribution).unwrap()];
    let synthesis_packets = vec![serde_json::from_value(synthesis).unwrap()];
    let proposals = vec![serde_json::from_value(proposal).unwrap()];
    let mut disposition = serde_json::json!({
        "schema_version": 1, "scope_id": "default", "id": "disposition_a",
        "proposal_id": "proposal_a", "decision": "accepted", "rationale": "Ratified the winner",
        "actor": {"identity_type": identity_type, "id": "reviewer"}
    });
    if names_artifact {
        disposition["canonical_artifact"] =
            serde_json::json!({"artifact_type": "resolution", "artifact_id": "res_a"});
    }
    let dispositions = vec![serde_json::from_value(disposition).unwrap()];

    crate::validate_ideation_aggregate(crate::IdeationAggregate {
        legacy_policy: crate::LegacyProposalPolicy::ModernOnly,
        ratification: crate::DispositionRatification::LegacyAllowlist(&["reviewer".into()]),
        contributions: &contributions,
        synthesis_packets: &synthesis_packets,
        proposals: &proposals,
        assertions: &[],
        dispositions: &dispositions,
    })
}

#[test]
fn human_acceptance_naming_a_ratified_artifact_needs_no_assertion() {
    validate_ratified_acceptance("human", true).unwrap();
}

#[test]
fn acceptance_stands_without_an_assertion_only_for_a_person_naming_an_artifact() {
    for (identity_type, names_artifact) in [
        ("human", false),
        ("agent", true),
        ("service", true),
        ("agent", false),
    ] {
        let error = validate_ratified_acceptance(identity_type, names_artifact)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("must be asserted"),
            "{identity_type} naming an artifact: {names_artifact}: {error}"
        );
    }
}

#[test]
fn accepted_disposition_still_requires_assertion() {
    let (contribution, synthesis, proposal, _) = lifecycle_fixture();
    let contributions = vec![serde_json::from_value(contribution).unwrap()];
    let synthesis_packets = vec![serde_json::from_value(synthesis).unwrap()];
    let proposals = vec![serde_json::from_value(proposal).unwrap()];
    let dispositions = vec![serde_json::from_value(serde_json::json!({
        "schema_version": 1, "scope_id": "default", "id": "disposition_a",
        "proposal_id": "proposal_a", "decision": "accepted", "rationale": "Reviewed",
        "actor": {"identity_type": "human", "id": "reviewer"}
    })).unwrap()];

    let error = crate::validate_ideation_aggregate(crate::IdeationAggregate {
        legacy_policy: crate::LegacyProposalPolicy::ModernOnly,
        ratification: crate::DispositionRatification::LegacyAllowlist(&["reviewer".into()]),
        contributions: &contributions,
        synthesis_packets: &synthesis_packets,
        proposals: &proposals,
        assertions: &[],
        dispositions: &dispositions,
    })
    .unwrap_err()
    .to_string();
    assert!(error.contains("must be asserted"), "{error}");
}

#[test]
fn rejected_or_deferred_disposition_can_override_asserted_state() {
    for decision in ["rejected", "deferred"] {
        let (contribution, synthesis, proposal, assertion) = lifecycle_fixture();
        let contributions = vec![serde_json::from_value(contribution).unwrap()];
        let synthesis_packets = vec![serde_json::from_value(synthesis).unwrap()];
        let proposals = vec![serde_json::from_value(proposal).unwrap()];
        let assertions = vec![serde_json::from_value(assertion).unwrap()];
        let dispositions = vec![serde_json::from_value(serde_json::json!({
            "schema_version": 1, "scope_id": "default", "id": format!("disposition_{decision}"),
            "proposal_id": "proposal_a", "decision": decision, "rationale": "Reviewed",
            "actor": {"identity_type": "human", "id": "reviewer"}
        })).unwrap()];

        crate::validate_ideation_aggregate(crate::IdeationAggregate {
            legacy_policy: crate::LegacyProposalPolicy::ModernOnly,
            ratification: crate::DispositionRatification::LegacyAllowlist(&["reviewer".into()]),
            contributions: &contributions,
            synthesis_packets: &synthesis_packets,
            proposals: &proposals,
            assertions: &assertions,
            dispositions: &dispositions,
        })
        .unwrap();
    }
}

#[test]
fn disposition_external_action_is_closed_and_round_trips_generically() {
    let value = serde_json::json!({
        "schema_version": 1,
        "scope_id": "default",
        "id": "disposition_a",
        "proposal_id": "proposal_a",
        "decision": "rejected",
        "rationale": "Implemented elsewhere",
        "actor": {"identity_type": "human", "id": "reviewer"},
        "external_action": {
            "system": "github",
            "scope": "acme/payroll",
            "kind": "commit",
            "key": "abc123"
        }
    });
    let disposition: crate::DispositionRecord = serde_json::from_value(value.clone()).unwrap();

    assert_eq!(serde_json::to_value(disposition).unwrap(), value);

    let mut unknown = value;
    unknown["external_action"]["workflow_state"] = serde_json::json!("deployed");
    let error = serde_json::from_value::<crate::DispositionRecord>(unknown)
        .unwrap_err()
        .to_string();
    assert!(error.contains("unknown field"), "{error}");
}

#[test]
fn disposition_external_action_rejects_null_and_blank_identity_parts() {
    let base = serde_json::json!({
        "schema_version": 1, "scope_id": "default", "id": "disposition_a",
        "proposal_id": "proposal_a", "decision": "rejected", "rationale": "Reviewed",
        "actor": {"identity_type": "human", "id": "reviewer"}
    });
    let mut null = base.clone();
    null["external_action"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<crate::DispositionRecord>(null).is_err());

    for field in ["system", "scope", "kind", "key"] {
        let mut value = base.clone();
        value["external_action"] = serde_json::json!({
            "system": "github", "scope": "acme/payroll", "kind": "issue", "key": "44"
        });
        value["external_action"][field] = serde_json::json!("  ");
        let disposition: crate::DispositionRecord = serde_json::from_value(value).unwrap();
        let error = crate::validate_disposition_intrinsic(&disposition)
            .unwrap_err()
            .to_string();
        assert!(error.contains(field), "{error}");
    }
}

/// Runs the whole aggregate for a live proposal, varying only what the rule
/// decides on: the manifest allowlist, the acting identity, and whether the
/// aggregate carries a disposition at all.
fn validate_allowlist_case(
    allowlist: &[String],
    actor_id: &str,
    carries_disposition: bool,
) -> anyhow::Result<()> {
    let (contribution, synthesis, proposal, assertion) = lifecycle_fixture();
    let contributions = vec![serde_json::from_value(contribution).unwrap()];
    let synthesis_packets = vec![serde_json::from_value(synthesis).unwrap()];
    let proposals = vec![serde_json::from_value(proposal).unwrap()];
    let assertions = vec![serde_json::from_value(assertion).unwrap()];
    let dispositions = if carries_disposition {
        vec![serde_json::from_value(serde_json::json!({
            "schema_version": 1, "scope_id": "default", "id": "disposition_a",
            "proposal_id": "proposal_a", "decision": "rejected", "rationale": "Reviewed",
            "actor": {"identity_type": "human", "id": actor_id}
        }))
        .unwrap()]
    } else {
        Vec::new()
    };

    crate::validate_ideation_aggregate(crate::IdeationAggregate {
        legacy_policy: crate::LegacyProposalPolicy::ModernOnly,
        ratification: crate::DispositionRatification::LegacyAllowlist(allowlist),
        contributions: &contributions,
        synthesis_packets: &synthesis_packets,
        proposals: &proposals,
        assertions: &assertions,
        dispositions: &dispositions,
    })
}

#[test]
#[verifies("rule_disposition_actor_allowlist", examples)]
fn allowlisted_actor_may_dispose_a_live_proposal() {
    validate_allowlist_case(&["auditor".into(), "reviewer".into()], "reviewer", true).unwrap();
}

#[test]
#[verifies("rule_disposition_actor_allowlist", examples)]
fn actor_outside_the_allowlist_cannot_dispose_a_live_proposal() {
    let error = validate_allowlist_case(&["reviewer".into()], "intruder", true)
        .unwrap_err()
        .to_string();
    // A populated allowlist gets the message it always got, with nothing
    // appended: the list is set, so the id really is the problem.
    assert_eq!(error, "disposition actor is not in the repository allowlist");
}

#[test]
#[verifies("rule_disposition_actor_allowlist", examples)]
fn empty_allowlist_rejects_every_disposition() {
    // Spelled out rather than read from the rule: an unlisted actor is told
    // that the list itself is unset, because no id would have passed.
    let expected = "no disposition actors configured: repository manifest \
                    disposition_actor_ids is empty; set it with \
                    provenance init --disposition-actor-id <ACTOR_ID>";
    for actor in ["reviewer", "auditor", "agent_1"] {
        let error = validate_allowlist_case(&[], actor, true)
            .unwrap_err()
            .to_string();
        assert_eq!(error, expected, "{actor}");
    }
}

#[test]
#[verifies("rule_disposition_actor_allowlist", examples)]
fn allowlist_ignores_actor_shaped_ids_on_other_record_kinds() {
    let (mut contribution, mut synthesis, proposal, assertion) = lifecycle_fixture();
    contribution["participant_slot"] = serde_json::json!("intruder");
    synthesis["suggested_artifacts"][0]["origin_participant_slots"] =
        serde_json::json!(["intruder"]);
    let contributions = vec![serde_json::from_value(contribution).unwrap()];
    let synthesis_packets = vec![serde_json::from_value(synthesis).unwrap()];
    let proposals = vec![serde_json::from_value(proposal).unwrap()];
    let assertions = vec![serde_json::from_value(assertion).unwrap()];

    crate::validate_ideation_aggregate(crate::IdeationAggregate {
        legacy_policy: crate::LegacyProposalPolicy::ModernOnly,
        ratification: crate::DispositionRatification::LegacyAllowlist(&["reviewer".into()]),
        contributions: &contributions,
        synthesis_packets: &synthesis_packets,
        proposals: &proposals,
        assertions: &assertions,
        dispositions: &[],
    })
    .unwrap();
}

#[test]
#[verifies("rule_disposition_actor_allowlist", exhaustion)]
fn allowlist_gates_disposition_actors_over_every_manifest() {
    // The decision ranges on two finite axes: whether the manifest lists the
    // acting identity, and whether the record deciding the proposal is a
    // disposition. Every allowlist over a two-actor universe is built from a
    // bitmask, so membership below is read off the construction rather than
    // re-derived from the allowlist the rule reads.
    const ACTORS: [&str; 2] = ["auditor", "reviewer"];
    for mask in 0..(1u8 << ACTORS.len()) {
        let allowlist = ACTORS
            .iter()
            .enumerate()
            .filter(|&(index, _)| mask & (1u8 << index) != 0)
            .map(|(_, actor)| (*actor).to_owned())
            .collect::<Vec<String>>();
        for (index, actor) in ACTORS.iter().enumerate() {
            let listed = mask & (1u8 << index) != 0;
            for carries_disposition in [false, true] {
                let outcome = validate_allowlist_case(&allowlist, actor, carries_disposition);
                // Independent restatement of the decision: only a disposition
                // is gated, and it passes exactly when the actor is listed.
                let expected_ok = !carries_disposition || listed;
                assert_eq!(
                    outcome.is_ok(),
                    expected_ok,
                    "allowlist {allowlist:?}, actor {actor}, \
                     disposition {carries_disposition}: {outcome:?}"
                );
            }
        }
    }
}
