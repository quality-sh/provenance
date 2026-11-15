//! Swarm runs that branches land, merged by `git merge` with the JSONL merge
//! driver of the built binary. Each merge result passes `provenance check`.

use crate::support::landing_runs::{
    contribution, contribution_ids, ideation_files, land, land_run, landing_repository,
    merge_into, proposal_run, read_families, read_record, Landing,
};
use crate::support::merge_driver::{commit, git, git_ok};
use provenance_macros::verifies;
use serde_json::Value;
use std::collections::BTreeMap;

/// The files and the reads of a merge of `from` into `into`, after the merge
/// passes with exit code 0.
fn clean_merge(
    landing: &Landing,
    into: &str,
    from: &str,
) -> (BTreeMap<String, Vec<u8>>, Vec<Value>) {
    let repo = landing.repo();
    let output = merge_into(repo, into, from);
    assert_eq!(
        output.status.code(),
        Some(0),
        "merge {from} into {into}: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    (ideation_files(repo), read_families(repo))
}

/// Merges in the two directions and gives the result after each one is the
/// same, byte for byte.
fn clean_in_each_direction(landing: &Landing, ours: &str, theirs: &str) -> Vec<Value> {
    let (files, reads) = clean_merge(landing, ours, theirs);
    let (other_files, other_reads) = clean_merge(landing, theirs, ours);
    assert!(files == other_files, "the two directions give the same files");
    assert_eq!(reads, other_reads);
    reads
}

/// A repository with the run `base` on `main`, the run of each name of
/// `theirs` on the branch `theirs`, and the run of each name of `ours` on
/// `main`.
fn two_branches(ours: &[&str], theirs: &[&str]) -> Landing {
    let landing = landing_repository();
    let repo = landing.repo();
    land(&landing, "base");
    commit(repo, "base");
    git_ok(repo, &["checkout", "-b", "theirs"]);
    for name in theirs {
        land(&landing, name);
    }
    commit(repo, "theirs");
    git_ok(repo, &["checkout", "main"]);
    for name in ours {
        land(&landing, name);
    }
    commit(repo, "ours");
    landing
}

#[test]
#[verifies("rule_swarm_run_records_in_kind_files", examples)]
#[verifies("rule_record_merge", examples)]
fn two_branches_that_land_different_runs_keep_the_records_of_each() {
    let landing = two_branches(&["ours"], &["theirs"]);
    let reads = clean_in_each_direction(&landing, "main", "theirs");
    assert_eq!(
        contribution_ids(&reads),
        ["contrib_base", "contrib_ours", "contrib_theirs"]
    );
}

#[test]
#[verifies("rule_swarm_run_records_in_kind_files", examples)]
#[verifies("rule_record_merge", examples)]
fn a_run_that_two_branches_land_merges_to_one_copy() {
    let landing = two_branches(&["same"], &["same", "theirs"]);
    let reads = clean_in_each_direction(&landing, "main", "theirs");
    assert_eq!(
        contribution_ids(&reads),
        ["contrib_base", "contrib_same", "contrib_theirs"]
    );
}

#[test]
#[verifies("rule_swarm_run_records_in_kind_files", examples)]
fn a_criss_cross_merge_of_two_merge_results_is_clean() {
    let landing = two_branches(&["ours"], &["theirs"]);
    let repo = landing.repo();
    for (name, into, from) in [("one", "main", "theirs"), ("two", "theirs", "main")] {
        git_ok(repo, &["checkout", "-B", name, into]);
        git_ok(repo, &["merge", from, "-m", name]);
        land(&landing, name);
        commit(repo, name);
    }
    let reads = clean_in_each_direction(&landing, "one", "two");
    assert_eq!(
        contribution_ids(&reads),
        [
            "contrib_base",
            "contrib_one",
            "contrib_ours",
            "contrib_theirs",
            "contrib_two"
        ]
    );
}

#[test]
#[verifies("rule_swarm_run_records_in_kind_files", examples)]
fn three_branches_merge_in_sequence() {
    let landing = landing_repository();
    let repo = landing.repo();
    land(&landing, "base");
    commit(repo, "base");
    for (branch, runs) in [
        ("first", ["first", "shared"]),
        ("second", ["second", "second"]),
        ("third", ["third", "shared"]),
    ] {
        git_ok(repo, &["checkout", "-b", branch, "main"]);
        for run in runs {
            land(&landing, run);
        }
        commit(repo, branch);
    }
    git_ok(repo, &["checkout", "main"]);
    for branch in ["first", "second", "third"] {
        let output = git(repo, &["merge", branch, "-m", branch]);
        assert_eq!(output.status.code(), Some(0), "merge {branch}");
    }
    assert_eq!(
        contribution_ids(&read_families(repo)),
        [
            "contrib_base",
            "contrib_first",
            "contrib_second",
            "contrib_shared",
            "contrib_third"
        ]
    );
}

#[test]
#[verifies("rule_swarm_run_records_in_kind_files", examples)]
#[verifies("rule_record_merge", examples)]
fn a_conflict_keeps_our_record_and_the_next_merge_is_clean() {
    let landing = landing_repository();
    let repo = landing.repo();
    let c = |finding: &str| contribution("contrib_c", finding);
    land_run(&landing, "base", &[c("Base.")], None);
    commit(repo, "base");
    git_ok(repo, &["checkout", "-b", "theirs"]);
    land_run(&landing, "theirs", &[c("Theirs."), contribution("contrib_d", "D.")], None);
    commit(repo, "theirs");
    git_ok(repo, &["checkout", "main"]);
    land_run(&landing, "ours", &[c("Ours.")], None);
    commit(repo, "ours");

    let output = merge_into(repo, "main", "theirs");
    assert_eq!(output.status.code(), Some(1));
    let text = [output.stdout, output.stderr].concat();
    let text = String::from_utf8_lossy(&text);
    assert!(
        text.contains("merge left 1 conflicting record(s): contrib_c"),
        "{text}"
    );
    commit(repo, "keep ours");
    let reads = read_families(repo);
    assert_eq!(contribution_ids(&reads), ["contrib_c", "contrib_d"]);
    assert_eq!(read_record(&reads[0], "contrib_c")["strongest_finding"], "Ours.");

    git_ok(repo, &["checkout", "theirs"]);
    land(&landing, "later");
    commit(repo, "later");
    git_ok(repo, &["checkout", "into_main"]);
    let output = git(repo, &["merge", "theirs", "-m", "again"]);
    assert_eq!(output.status.code(), Some(0));
    let reads = read_families(repo);
    assert_eq!(
        contribution_ids(&reads),
        ["contrib_c", "contrib_d", "contrib_later"]
    );
    assert_eq!(read_record(&reads[0], "contrib_c")["strongest_finding"], "Ours.");
}

/// One branch asserts a proposal on the claim of `contrib_c`. The other
/// branch changes `contrib_c` and keeps the claim. The merge equals the
/// history "change, then assert", and its state passes the check.
#[test]
#[verifies("rule_swarm_run_records_in_kind_files", examples)]
#[verifies("rule_record_merge", examples)]
fn an_assertion_and_a_change_of_its_evidence_on_the_other_branch_merge_clean() {
    let landing = landing_repository();
    let repo = landing.repo();
    land_run(&landing, "base", &[contribution("contrib_c", "Base.")], None);
    commit(repo, "base");
    git_ok(repo, &["checkout", "-b", "theirs"]);
    land_run(&landing, "change", &[contribution("contrib_c", "Changed.")], None);
    commit(repo, "change");
    git_ok(repo, &["checkout", "main"]);
    land_run(&landing, "assert", &[], Some(proposal_run("P", "claim_contrib_c")));
    commit(repo, "assert");

    let reads = clean_in_each_direction(&landing, "main", "theirs");
    assert_eq!(read_record(&reads[0], "contrib_c")["strongest_finding"], "Changed.");
    assert_eq!(
        read_record(&reads[3], "assertion_p")["supporting_claim_ids"],
        serde_json::json!(["claim_contrib_c"])
    );
}
