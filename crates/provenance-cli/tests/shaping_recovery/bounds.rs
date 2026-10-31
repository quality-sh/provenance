use super::*;

#[test]
#[verifies("rule_shaping_recovery_bounds", examples)]
fn summary_has_a_byte_bound_when_claims_are_large() {
    let (_dir, repo) = setup();
    // Each actor stays far below the 32,767-character Windows command-line limit.
    let actor = "worker".repeat(3000);
    let mut claimed = 0;
    for id in [
        "question_a",
        "question_b",
        "question_c",
        "question_d",
        "question_e",
        "question_f",
    ] {
        create(&repo, id, None);
        let claim = [
            "questions",
            id,
            "claim",
            "--repo",
            &repo,
            "--scope",
            "default",
            "--actor",
            &actor,
        ];
        provenance(&claim).success();
        claimed += get(&repo, "questions", id)["data"]["claimed_by"]
            .as_str()
            .unwrap()
            .len();
    }
    assert!(claimed > 65_536, "claim text is only {claimed} bytes");
    let state = summary(&repo, &[]);
    assert_eq!(state["complete"], false);
    assert!(serde_json::to_vec(&state).unwrap().len() <= 65_536);
}
