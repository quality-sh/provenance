//! Verification of `rule_install_run_status`, plus the local unit tests for
//! the installer's helpers.

use provenance_macros::verifies;

use super::*;

/// Every outcome a file report can carry, walked off the production enum
/// itself: the chain below matches on `FileStatus` exhaustively, so a sixth
/// outcome added to the installer's vocabulary fails to compile until it
/// takes its place in the walk and, through it, in the run below.
fn all_outcomes() -> Vec<FileStatus> {
    let mut all = vec![FileStatus::Unchanged];
    while let Some(next) = match all.last().unwrap() {
        FileStatus::Unchanged => Some(FileStatus::Installed),
        FileStatus::Installed => Some(FileStatus::Updated),
        FileStatus::Updated => Some(FileStatus::Linked),
        FileStatus::Linked => Some(FileStatus::Removed),
        FileStatus::Removed => None,
    } {
        all.push(next);
    }
    all
}

/// Independent restatement of the rule, used as the oracle below: rank each
/// file by how much it changed and report the strongest rank present. It
/// must not be implemented by calling `combined_status`.
fn strength(outcome: FileStatus) -> usize {
    match outcome {
        FileStatus::Unchanged => 0,
        FileStatus::Installed | FileStatus::Linked => 1,
        FileStatus::Updated | FileStatus::Removed => 2,
    }
}

fn strongest(outcomes: &[FileStatus]) -> FileStatus {
    let rank = outcomes.iter().copied().map(strength).max().unwrap_or(0);
    [
        FileStatus::Unchanged,
        FileStatus::Installed,
        FileStatus::Updated,
    ][rank]
}

fn reports(outcomes: &[FileStatus]) -> Vec<FileInstallReport> {
    outcomes
        .iter()
        .enumerate()
        .map(|(index, outcome)| FileInstallReport {
            path: format!("file-{index}"),
            status: *outcome,
        })
        .collect()
}

/// `combined_status` reads its argument only through `any(...)` scans that
/// test one file's status at a time, so its answer is a function of *which*
/// outcomes appear, never of how many files carry an outcome nor of the
/// order they arrive in. The set of outcomes present is therefore the entire
/// input as far as the classification can see, and the powerset of the
/// `FileStatus` variants - 32 cases today, the empty run included - exhausts
/// an otherwise unbounded domain of file lists.
///
/// The invariance that licenses that bound is checked rather than assumed:
/// each case is also run reversed and with every outcome appearing twice,
/// and must classify the same.
#[test]
#[verifies("rule_install_run_status", exhaustion)]
fn every_outcome_set_reports_the_strongest_change() {
    let outcomes = all_outcomes();
    for mask in 0..(1usize << outcomes.len()) {
        let present = outcomes
            .iter()
            .copied()
            .enumerate()
            .filter(|(index, _)| (mask >> index) & 1 == 1)
            .map(|(_, outcome)| outcome)
            .collect::<Vec<_>>();
        let padded = present
            .iter()
            .rev()
            .chain(present.iter())
            .copied()
            .collect::<Vec<_>>();
        let expected = strongest(&present);
        for files in [reports(&present), reports(&padded)] {
            let statuses = files.iter().map(|file| file.status).collect::<Vec<_>>();
            assert_eq!(
                combined_status(&files),
                expected,
                "run of {statuses:?} should be reported as {expected:?}"
            );
        }
    }
}

#[test]
fn home_dir_uses_userprofile_when_home_is_absent() {
    let resolved = home_dir_from_env(|key| {
        if key == "USERPROFILE" {
            Some(OsString::from(r"C:\Users\Ada"))
        } else {
            None
        }
    })
    .unwrap();

    assert_eq!(resolved, PathBuf::from(r"C:\Users\Ada"));
}
