//! Record history over a committed state line that holds a member two times,
//! and over a committed line of another shape than the current record type.
//!
//! The history read skips a committed line that holds a top-level member two
//! times, as it skips a committed conflict marker. The two tests of that skip
//! have no Rule binding: the text of `rule_record_history_reads_git` does not
//! state what the history gives for a committed line that it cannot read as
//! a record. A committed line of another record shape, or with the removed
//! `retired` member, is a committed version of the record, as that Rule
//! states.

use crate::review_support;
use provenance_macros::verifies;
use review_support::*;

/// The commits of the history of the Requirement `record` at `temp`.
async fn commits(temp: &tempfile::TempDir, record: &str) -> Vec<String> {
    history(temp, record)
        .await
        .into_iter()
        .map(|version| version.id.as_str().to_owned())
        .collect()
}

/// Commits the Requirement file with the `req_a` line changed by `change`,
/// then commits the file as it was. Returns the commit that holds the changed
/// line and the commit that restores it.
fn commit_changed_line(
    temp: &tempfile::TempDir,
    change: impl Fn(&str) -> String,
) -> (String, String) {
    let path = requirements(temp);
    let text = std::fs::read_to_string(&path).unwrap();
    let changed = text.lines().fold(String::new(), |mut changed, line| {
        if line.contains(r#""id":"req_a""#) {
            changed.push_str(&change(line));
        } else {
            changed.push_str(line);
        }
        changed.push('\n');
        changed
    });
    assert_ne!(changed, text, "no line of {path} holds req_a");
    std::fs::write(&path, changed).unwrap();
    let commit = commit_state(temp, "Commit the changed line");
    std::fs::write(&path, &text).unwrap();
    (commit, commit_state(temp, "Restore the line"))
}

#[tokio::test]
async fn history_skips_a_committed_line_whose_last_id_names_the_record() {
    let (temp, _store) = fixture();
    let created = commit_state(&temp, "Create");
    commit_changed_line(&temp, |line| {
        line.replacen('{', r#"{"id":"req_shadow","#, 1).replace(
            "The system stores records.",
            "The shadow line holds this text.",
        )
    });

    assert_eq!(commits(&temp, "req_a").await, [created]);
}

#[tokio::test]
async fn history_skips_a_committed_line_whose_first_id_names_the_record() {
    let (temp, _store) = fixture();
    commit_state(&temp, "Create");
    commit_changed_line(&temp, |line| {
        line.replace(r#""id":"req_a""#, r#""id":"req_a","id":"req_shadow""#)
    });

    assert_eq!(commits(&temp, "req_shadow").await, [] as [String; 0]);
}

#[tokio::test]
#[verifies("rule_record_history_reads_git", examples)]
async fn history_shows_a_committed_version_of_another_record_shape() {
    let (temp, _store) = fixture();
    let created = commit_state(&temp, "Create");
    let (changed, restore) =
        commit_changed_line(&temp, |line| line.replace(r#""statement":"#, r#""text":"#));

    assert_eq!(commits(&temp, "req_a").await, [created, changed, restore]);
}

#[tokio::test]
#[verifies("rule_record_history_reads_git", examples)]
async fn history_shows_a_committed_version_that_holds_retired() {
    let (temp, _store) = fixture();
    let created = commit_state(&temp, "Create");
    let (changed, restore) =
        commit_changed_line(&temp, |line| line.replacen('{', r#"{"retired":false,"#, 1));

    assert_eq!(commits(&temp, "req_a").await, [created, changed, restore]);
}
