//! Each branch submits a review of one record from one base, so the two
//! submissions have one review cycle. The merge driver reports a conflict
//! and writes both submissions. A person who accepts each file that the
//! driver wrote gets a read of the record that refuses and names each line
//! of each review file to remove. After that repair, the record reads.

use super::review_cycle::describe;
use super::review_disagreeing_rows::{check_reports, line_of, refused, remove_lines, state_file};
use super::review_history::{commit, git, git_output};
use super::review_merged_decisions::{decide, lines, DISPOSITIONS, PROPOSALS, WITHDRAWALS};
use super::review_merged_withdrawals::{check_passes, pending, read, repository, withdraw, Merge};
use super::*;

/// The ID of a stored line.
fn id_of(line: &str) -> String {
    serde_json::from_str::<Value>(line).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// The Proposal file that the merge driver wrote holds each line of our
/// branch and each line that only their branch added, in ID order. Gives
/// the files that the merge left unmerged.
pub(super) fn driver_kept_both_sides(repo: &str) -> Vec<String> {
    let conflicts = git(repo, &["diff", "--name-only", "--diff-filter=U"]);
    let conflicts = conflicts.lines().map(str::to_owned).collect::<Vec<_>>();
    assert!(
        conflicts.iter().any(|file| file == PROPOSALS),
        "{conflicts:?}"
    );
    let ours = git(repo, &["show", &format!(":2:{PROPOSALS}")]);
    let theirs = git(repo, &["show", &format!(":3:{PROPOSALS}")]);
    let mut expected = ours.lines().map(str::to_owned).collect::<Vec<_>>();
    let added = theirs
        .lines()
        .filter(|line| !ours.lines().any(|our| our == *line));
    expected.extend(added.map(str::to_owned));
    expected.sort_by_key(|line| id_of(line));
    assert_eq!(lines(repo, PROPOSALS), expected);
    conflicts
}

/// The line of the row of `file` whose `field` is `id`.
fn line_naming(repo: &str, file: &str, field: &str, id: &str) -> usize {
    line_of(repo, file, &format!("\"{field}\":\"{id}\""))
}

/// Flow: each branch changes `req_a`, which opens a review submission with
/// review cycle 2 on each branch. Their branch then withdraws or rejects its
/// submission. Git merges with the driver, and the person accepts each file
/// that the driver wrote and commits. The read of `req_a` refuses and names
/// the lines of each submission, also the withdrawal or the decision of
/// their submission. After the person removes the lines of their
/// submission, `req_a` reads with our submission pending.
#[test]
#[verifies("rule_disagreeing_review_rows_refuse_record", examples)]
#[verifies("rule_review_row_refusal_gives_repair", examples)]
fn accepted_driver_files_of_two_submissions_with_one_cycle_refuse_with_the_repair() {
    for (side, file, field) in [
        ("withdraw", WITHDRAWALS, "proposal_id"),
        ("rejected", DISPOSITIONS, "proposal_id"),
    ] {
        let (_directory, repo) = repository(Merge::Driver);
        let first = git(&repo, &["rev-parse", "--abbrev-ref", "HEAD"]);
        git(&repo, &["checkout", "-q", "-b", "side"]);
        describe(&repo, "req_a", "The change of the side branch.");
        let theirs = pending(&repo, "req_a");
        if side == "withdraw" {
            withdraw(&repo, "req_a", &theirs, "agent", "Side.");
        } else {
            decide(&repo, "req_a", &theirs, side, "Side.", None);
        }
        commit(&repo);
        git(&repo, &["checkout", "-q", &first]);
        describe(&repo, "req_a", "The change of the first branch.");
        let ours = pending(&repo, "req_a");
        commit(&repo);

        let merged = git_output(&repo, &["merge", "--no-edit", "side"]);
        assert!(!merged.status.success(), "{merged:?}");
        for conflict in driver_kept_both_sides(&repo) {
            git(&repo, &["add", "--", &conflict]);
        }
        commit(&repo);

        let our_line = line_naming(&repo, PROPOSALS, "id", &ours);
        let their_line = line_naming(&repo, PROPOSALS, "id", &theirs);
        let named = line_naming(&repo, file, field, &theirs);
        let their_lines = format!(
            "ideation/proposal_cards.jsonl line {their_line}; {} line {named}",
            state_file(file).trim_start_matches("scopes/default/")
        );
        let our_lines = format!("ideation/proposal_cards.jsonl line {our_line}");
        let mut each = [
            (our_line, &ours, our_lines),
            (their_line, &theirs, their_lines),
        ];
        each.sort();
        let expected = json!({"kind":"invalid_state_file", "file":state_file(PROPOSALS),
            "line":each[1].0,
            "message":format!("requirement req_a has 2 review submissions with review cycle \
                2. The lines of {}: {}. The lines of {}: {}", each[0].1, each[0].2, each[1].1,
                each[1].2),
            "hint":"To repair, keep one submission and remove the lines of each other \
                submission, then commit."});
        assert_eq!(
            refused(&repo, &["api", "requirements/req_a"]),
            expected,
            "{side}"
        );
        check_reports(&repo, &expected);

        remove_lines(&repo, &[(PROPOSALS, their_line), (file, named)]);
        let state = read(&repo, &["api", "requirements/req_a"])["data"]["decision"].clone();
        assert_eq!(state["pending"]["proposal_id"], json!(ours), "{side}");
        check_passes(&repo);
    }
}
