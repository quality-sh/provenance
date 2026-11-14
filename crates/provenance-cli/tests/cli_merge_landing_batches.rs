use crate::support::landing_runs::{
    contribution, contribution_ids, driver_outcome, land, land_run, landing_lines,
    landing_repository, merge_into, proposal_run, read_families,
};
use crate::support::merge_driver::{commit, git_ok, json};
use provenance_macros::verifies;
use serde_json::Value;

#[test]
#[verifies("rule_landing_batch_merge", examples)]
fn the_merge_keeps_the_different_batches_of_each_side_with_one_read_in_each_direction() {
    let landing = landing_repository();
    let repo = landing.repo();
    let base = land(&landing, "base");
    commit(repo, "base");
    git_ok(repo, &["checkout", "-b", "theirs"]);
    let theirs = land(&landing, "theirs");
    commit(repo, "theirs");
    git_ok(repo, &["checkout", "main"]);
    let ours = land(&landing, "ours");
    commit(repo, "ours");

    assert_eq!(merge_into(repo, "main", "theirs").status.code(), Some(0));
    assert_eq!(
        landing_lines(repo),
        [base.clone(), ours.clone(), theirs.clone()]
    );
    let into_ours = read_families(repo);
    assert_eq!(
        contribution_ids(&into_ours),
        ["contrib_base", "contrib_ours", "contrib_theirs"]
    );
    assert_eq!(merge_into(repo, "theirs", "main").status.code(), Some(0));
    assert_eq!(landing_lines(repo), [base, theirs, ours]);
    assert_eq!(read_families(repo), into_ours);
}

#[test]
#[verifies("rule_landing_batch_merge", examples)]
fn the_merge_keeps_a_batch_that_the_two_sides_add_one_time() {
    let landing = landing_repository();
    let repo = landing.repo();
    let base = land(&landing, "base");
    commit(repo, "base");
    git_ok(repo, &["checkout", "-b", "theirs"]);
    let same = land(&landing, "same");
    let theirs = land(&landing, "theirs");
    commit(repo, "theirs");
    git_ok(repo, &["checkout", "main"]);
    assert_eq!(land(&landing, "same"), same);
    commit(repo, "ours");

    assert_eq!(merge_into(repo, "main", "theirs").status.code(), Some(0));
    assert_eq!(landing_lines(repo), [base, same, theirs]);
    assert_eq!(
        contribution_ids(&read_families(repo)),
        ["contrib_base", "contrib_same", "contrib_theirs"]
    );
}

#[test]
#[verifies("rule_landing_batch_merge", examples)]
#[verifies("rule_record_merge", examples)]
fn a_record_that_two_batches_hold_with_equal_content_merges_to_one_read_in_each_direction() {
    let landing = landing_repository();
    let repo = landing.repo();
    let base = land(&landing, "base");
    commit(repo, "base");
    git_ok(repo, &["checkout", "-b", "theirs"]);
    let same = contribution("contrib_same", "Equal on the two sides.");
    let theirs = land_run(
        &landing,
        "theirs",
        &[same.clone(), contribution("contrib_theirs", "Of theirs.")],
        None,
    );
    commit(repo, "theirs");
    git_ok(repo, &["checkout", "-b", "ours", "main"]);
    let changed = contribution("contrib_base", "Changed by ours only.");
    let ours = land_run(
        &landing,
        "ours",
        &[changed, same, contribution("contrib_ours", "Of ours.")],
        None,
    );
    commit(repo, "ours");

    assert_eq!(merge_into(repo, "ours", "theirs").status.code(), Some(0));
    assert_eq!(
        landing_lines(repo),
        [base.clone(), ours.clone(), theirs.clone()]
    );
    let into_ours = read_families(repo);
    assert_eq!(merge_into(repo, "theirs", "ours").status.code(), Some(0));
    assert_eq!(landing_lines(repo), [base, theirs, ours]);
    assert_eq!(read_families(repo), into_ours);
    assert_eq!(
        contribution_ids(&into_ours),
        [
            "contrib_base",
            "contrib_ours",
            "contrib_same",
            "contrib_theirs"
        ]
    );
    assert_eq!(
        into_ours[0][0]["strongest_finding"],
        "Changed by ours only."
    );
}

#[test]
#[verifies("rule_record_merge", examples)]
fn two_batches_that_add_one_proposal_differently_conflict_and_keep_ours() {
    let landing = landing_repository();
    let repo = landing.repo();
    let base = land(&landing, "base");
    commit(repo, "base");
    let evidence = [contribution("contrib_p", "The claim of the proposal.")];
    git_ok(repo, &["checkout", "-b", "theirs"]);
    land_run(
        &landing,
        "theirs",
        &evidence,
        Some(proposal_run("Of theirs", "claim_contrib_p")),
    );
    commit(repo, "theirs");
    git_ok(repo, &["checkout", "main"]);
    let ours = land_run(&landing, "ours", &evidence, Some(proposal_run("Of ours", "claim_contrib_p")));
    commit(repo, "ours");

    let merge = merge_into(repo, "main", "theirs");
    assert_eq!(merge.status.code(), Some(1));
    let (code, outcome, _) = driver_outcome(&landing);
    assert_eq!(code, Some(1));
    let conflicts = outcome["conflicts"].as_array().unwrap();
    assert_eq!(conflicts.len(), 1, "{outcome}");
    assert_eq!(conflicts[0]["kind"], "add_add");
    assert_eq!(conflicts[0]["record_id"], "proposals/proposal_p");
    assert_eq!(conflicts[0]["base"], Value::Null);
    assert_eq!(conflicts[0]["ours"]["title"], "Of ours");
    assert_eq!(conflicts[0]["theirs"]["title"], "Of theirs");
    assert_eq!(landing_lines(repo), [base, ours]);
    let read = json(repo, &["api", "proposals"]);
    let proposal = read["data"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == "proposal_p")
        .unwrap();
    assert_eq!(proposal["title"], "Of ours", "{read}");
}
