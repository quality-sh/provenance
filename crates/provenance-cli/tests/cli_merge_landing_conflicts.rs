use crate::support::landing_runs::{
    contribution, driver_outcome, land, land_run, landing_lines, landing_repository, landings,
    merge_into, proposal_run, read_families, read_record, Landing, LANDINGS,
};
use crate::support::merge_driver::{commit, git_ok, json};
use provenance_macros::verifies;
use serde_json::{json, Value};

/// The next step that the text of each conflict of the landings file gives.
const NEXT_STEP: &str = "Git keeps their version of the file as stage 3 of the path. When a \
batch of theirs comes from a swarm run, `provenance swarm-backtrace land --replace --run-dir \
<run>` lands the run again, and each contribution and synthesis packet of the run then has the \
record of theirs. The landing refuses the run when the file holds a proposal, an assertion or a \
disposition with an id of the run, or when an assertion of the file cites a contribution or a \
synthesis packet that the run changes. No command of Provenance replaces a proposal, an \
assertion or a disposition that the file holds, or a contribution or a synthesis packet that an \
assertion of the file cites. After the change, mark the path as resolved.";

/// The first line of the text of each conflict of the landings file.
const FIRST_LINE: &str = "the merge of the landing batches has a conflict, so the file holds \
the batches of our side";

/// Merges `from` into a branch at `into`, and checks that the merge is a
/// conflict that leaves the file of `into` as it is and a scope that passes
/// the check. Returns the outcome and the standard error of the driver.
fn conflict_keeps_ours(landing: &Landing, into: &str, from: &str) -> (Value, String) {
    let repo = landing.repo();
    git_ok(repo, &["checkout", into]);
    let ours = landing_lines(repo);
    let merge = merge_into(repo, into, from);
    assert_eq!(merge.status.code(), Some(1), "into {into}");
    let (code, outcome, stderr) = driver_outcome(landing);
    assert_eq!(code, Some(1));
    assert_eq!(outcome["status"], "conflicted");
    assert!(stderr.contains(FIRST_LINE), "{stderr}");
    assert!(stderr.contains(NEXT_STEP), "{stderr}");
    assert_eq!(landing_lines(repo), ours, "into {into}");
    read_families(repo);
    (outcome, stderr)
}

fn abort(landing: &Landing) {
    git_ok(landing.repo(), &["merge", "--abort"]);
}

/// The record ids of the batches of theirs that the outcome names as left
/// out of the file, with the row of each batch in their file.
fn left_out(outcome: &Value) -> Value {
    outcome.get("left_out").cloned().unwrap_or(Value::Null)
}

#[test]
#[verifies("rule_landing_batch_merge_conflict", examples)]
fn a_record_that_each_side_changes_differently_around_a_common_batch_conflicts() {
    let landing = landing_repository();
    let repo = landing.repo();
    land(&landing, "base");
    commit(repo, "base");
    git_ok(repo, &["checkout", "-b", "theirs"]);
    let their_record = contribution("contrib_c", "Finding of theirs.");
    land_run(
        &landing,
        "theirs",
        std::slice::from_ref(&their_record),
        None,
    );
    let shared = land(&landing, "shared");
    commit(repo, "theirs");
    git_ok(repo, &["checkout", "-b", "ours", "main"]);
    assert_eq!(land(&landing, "shared"), shared);
    let our_record = contribution("contrib_c", "Finding of ours.");
    land_run(&landing, "ours", std::slice::from_ref(&our_record), None);
    commit(repo, "ours");

    for (into, from, row) in [("ours", "theirs", 2), ("theirs", "ours", 3)] {
        let (outcome, stderr) = conflict_keeps_ours(&landing, into, from);
        let (ours, theirs) = if into == "ours" {
            (&our_record, &their_record)
        } else {
            (&their_record, &our_record)
        };
        assert_eq!(
            outcome["conflicts"],
            json!([{"kind": "add_add", "record_id": "contributions/contrib_c",
                "base": null, "ours": ours, "theirs": theirs}]),
            "into {into}"
        );
        assert_eq!(
            left_out(&outcome),
            json!([{"row": row, "record_ids": ["contributions/contrib_c"]}]),
            "into {into}"
        );
        let text = format!(
            "Error: {FIRST_LINE}\nrecords that the two sides change differently: \
             contributions/contrib_c\nbatches of theirs that the file does not hold: row {row} \
             of theirs: contributions/contrib_c\n{NEXT_STEP}\n"
        );
        assert_eq!(stderr, text, "into {into}");
        let read = json(repo, &["api", "contributions"]);
        assert_eq!(read_record(&read["data"]["items"], "contrib_c"), ours);
        abort(&landing);
    }
}

#[test]
#[verifies("rule_landing_batch_merge_conflict", examples)]
fn one_proposal_with_equal_content_in_two_different_batches_conflicts_in_each_direction() {
    let landing = landing_repository();
    let repo = landing.repo();
    land(&landing, "base");
    commit(repo, "base");
    let evidence = contribution("contrib_p", "The claim of the proposal.");
    git_ok(repo, &["checkout", "-b", "theirs"]);
    let run = || Some(proposal_run("Same title", "claim_contrib_p"));
    land_run(&landing, "theirs", std::slice::from_ref(&evidence), run());
    commit(repo, "theirs");
    git_ok(repo, &["checkout", "-b", "ours", "main"]);
    let extra = contribution("contrib_extra", "One more finding.");
    land_run(&landing, "ours", &[evidence, extra], run());
    commit(repo, "ours");

    for (into, from) in [("ours", "theirs"), ("theirs", "ours")] {
        let (outcome, stderr) = conflict_keeps_ours(&landing, into, from);
        assert_eq!(outcome["conflicts"], json!([]), "into {into}");
        let check = |order: &str, side: &str| {
            json!({"order": order, "side": side, "row": 2,
                "record_id": "proposals/proposal_p",
                "check": "duplicate immutable proposal id proposal_p"})
        };
        assert_eq!(
            outcome["failed_checks"],
            json!([check("ours_first", "theirs"), check("theirs_first", "ours")]),
            "into {into}"
        );
        assert!(
            stderr.contains(
                "checks that the merged file fails: row 2 of theirs with our batches first: \
                 duplicate immutable proposal id proposal_p; row 2 of ours with their batches \
                 first: duplicate immutable proposal id proposal_p"
            ),
            "{stderr}"
        );
        abort(&landing);
    }
}

#[test]
#[verifies("rule_landing_batch_merge_conflict", examples)]
fn an_assertion_of_one_side_on_a_contribution_that_the_other_side_replaces_conflicts() {
    let landing = landing_repository();
    let repo = landing.repo();
    let base = contribution("contrib_c", "Finding of the base.");
    land_run(&landing, "base", &[base], None);
    commit(repo, "base");
    git_ok(repo, &["checkout", "-b", "theirs"]);
    land_run(
        &landing,
        "theirs",
        &[],
        Some(proposal_run("Asserted", "claim_contrib_c")),
    );
    commit(repo, "theirs");
    git_ok(repo, &["checkout", "-b", "ours", "main"]);
    let replaced = contribution("contrib_c", "Finding of ours.");
    land_run(&landing, "base", &[replaced], None);
    commit(repo, "ours");

    for (into, from, order, side) in [
        ("ours", "theirs", "theirs_first", "ours"),
        ("theirs", "ours", "ours_first", "theirs"),
    ] {
        let (outcome, _) = conflict_keeps_ours(&landing, into, from);
        assert_eq!(
            outcome["failed_checks"],
            json!([{"order": order, "side": side, "row": 2,
                "record_id": "contributions/contrib_c",
                "check": "contribution contrib_c is referenced by an assertion and cannot be \
                    replaced"}]),
            "into {into}"
        );
        abort(&landing);
    }
}

#[test]
#[verifies("rule_landing_batch_merge_conflict", examples)]
#[verifies("rule_record_merge", examples)]
fn a_batch_of_theirs_with_one_conflicting_record_is_named_with_each_of_its_records() {
    let landing = landing_repository();
    let repo = landing.repo();
    let base_record = contribution("contrib_c", "Finding of the base.");
    land_run(&landing, "base", std::slice::from_ref(&base_record), None);
    commit(repo, "base");
    git_ok(repo, &["checkout", "-b", "theirs"]);
    let their_record = contribution("contrib_c", "Finding of theirs.");
    let other = contribution("contrib_d", "No conflict.");
    land_run(&landing, "theirs", &[their_record.clone(), other], None);
    commit(repo, "theirs");
    git_ok(repo, &["checkout", "-b", "ours", "main"]);
    let our_record = contribution("contrib_c", "Finding of ours.");
    land_run(&landing, "ours", std::slice::from_ref(&our_record), None);
    commit(repo, "ours");

    let (outcome, stderr) = conflict_keeps_ours(&landing, "ours", "theirs");
    assert_eq!(
        outcome["conflicts"],
        json!([{"kind": "divergent_edit", "record_id": "contributions/contrib_c",
            "base": base_record, "ours": our_record, "theirs": their_record}])
    );
    assert_eq!(
        left_out(&outcome),
        json!([{"row": 2, "record_ids": ["contributions/contrib_c", "contributions/contrib_d"]}])
    );
    let text = format!(
        "Error: {FIRST_LINE}\nrecords that the two sides change differently: \
         contributions/contrib_c\nbatches of theirs that the file does not hold: row 2 of \
         theirs: contributions/contrib_c, contributions/contrib_d\n{NEXT_STEP}\n"
    );
    assert_eq!(stderr, text);
    let read = json(repo, &["api", "contributions"]);
    assert_eq!(
        read_record(&read["data"]["items"], "contrib_c"),
        &our_record
    );
}

#[test]
#[verifies("rule_landing_batch_merge_conflict", examples)]
fn the_next_step_of_the_conflict_text_lands_the_run_of_theirs_again() {
    let landing = landing_repository();
    let repo = landing.repo();
    land_run(
        &landing,
        "base",
        &[contribution("contrib_c", "Base.")],
        None,
    );
    commit(repo, "base");
    git_ok(repo, &["checkout", "-b", "theirs"]);
    let their_record = contribution("contrib_c", "Finding of theirs.");
    let other = contribution("contrib_d", "No conflict.");
    land_run(
        &landing,
        "theirs",
        &[their_record.clone(), other.clone()],
        None,
    );
    commit(repo, "theirs");
    git_ok(repo, &["checkout", "-b", "ours", "main"]);
    land_run(
        &landing,
        "ours",
        &[contribution("contrib_c", "Ours.")],
        None,
    );
    commit(repo, "ours");
    conflict_keeps_ours(&landing, "ours", "theirs");

    let run = landing.run_dir("theirs");
    json(
        repo,
        &[
            "swarm-backtrace",
            "land",
            "--replace",
            "--run-dir",
            run.to_str().unwrap(),
        ],
    );
    git_ok(repo, &["add", LANDINGS]);
    git_ok(repo, &["commit", "--no-edit"]);

    let reads = read_families(repo);
    assert_eq!(read_record(&reads[0], "contrib_c"), &their_record);
    assert_eq!(read_record(&reads[0], "contrib_d"), &other);
}

#[test]
#[verifies("rule_landing_batch_merge_conflict", examples)]
fn a_side_that_lands_a_base_batch_again_conflicts_with_a_change_of_the_other_side() {
    let landing = landing_repository();
    let repo = landing.repo();
    let base_record = contribution("contrib_c", "Finding of the base.");
    let base = land_run(&landing, "base", std::slice::from_ref(&base_record), None);
    commit(repo, "base");
    git_ok(repo, &["checkout", "-b", "theirs"]);
    let their_record = contribution("contrib_c", "Finding of theirs.");
    land_run(
        &landing,
        "theirs",
        std::slice::from_ref(&their_record),
        None,
    );
    commit(repo, "theirs");
    git_ok(repo, &["checkout", "-b", "ours", "main"]);
    let again = land_run(&landing, "base", std::slice::from_ref(&base_record), None);
    assert_eq!(again, base);
    commit(repo, "ours");

    let (outcome, _) = conflict_keeps_ours(&landing, "ours", "theirs");
    assert_eq!(
        outcome["conflicts"],
        json!([{"kind": "divergent_edit", "record_id": "contributions/contrib_c",
            "base": base_record, "ours": base_record, "theirs": their_record}])
    );
    assert_eq!(
        left_out(&outcome),
        json!([{"row": 2, "record_ids": ["contributions/contrib_c"]}])
    );
}

/// The product does not remove a batch, so this test is not bound to a
/// Rule: it shows what the driver does with a side that a person edited.
#[test]
fn a_batch_of_the_base_that_one_side_removes_by_hand_is_a_conflict() {
    let landing = landing_repository();
    let repo = landing.repo();
    let kept = land(&landing, "kept");
    land(&landing, "removed");
    commit(repo, "base");
    git_ok(repo, &["checkout", "-b", "theirs"]);
    land(&landing, "theirs");
    commit(repo, "theirs");
    git_ok(repo, &["checkout", "-b", "ours", "main"]);
    std::fs::write(landings(repo), format!("{kept}\n")).unwrap();
    commit(repo, "ours");

    let (outcome, stderr) = conflict_keeps_ours(&landing, "ours", "theirs");
    assert_eq!(outcome["conflicts"], json!([]));
    assert_eq!(outcome["sequence"], json!({"ours": null, "theirs": 2}));
    assert_eq!(
        left_out(&outcome),
        json!([
            {"row": 2, "record_ids": ["contributions/contrib_removed"]},
            {"row": 3, "record_ids": ["contributions/contrib_theirs"]}
        ])
    );
    assert!(
        stderr.contains(
            "common batches that the two sides hold in different sequences, from the end of \
             ours and row 2 of theirs"
        ),
        "{stderr}"
    );
}

#[test]
#[verifies("rule_landing_batch_merge_conflict", examples)]
fn common_batches_in_different_sequences_conflict() {
    let landing = landing_repository();
    let repo = landing.repo();
    land(&landing, "base");
    commit(repo, "base");
    git_ok(repo, &["checkout", "-b", "theirs"]);
    let first = land(&landing, "first");
    let second = land(&landing, "second");
    commit(repo, "theirs");
    git_ok(repo, &["checkout", "-b", "ours", "main"]);
    assert_eq!(land(&landing, "second"), second);
    assert_eq!(land(&landing, "first"), first);
    commit(repo, "ours");

    let (outcome, _) = conflict_keeps_ours(&landing, "ours", "theirs");
    assert_eq!(outcome["conflicts"], json!([]));
    assert_eq!(outcome["sequence"], json!({"ours": 2, "theirs": 2}));
    assert_eq!(left_out(&outcome), Value::Null);
}
