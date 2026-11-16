//! Record history over a committed state line that holds a member two times,
//! and over a committed line of another shape than the current record type.
//!
//! The history read skips a committed line that holds a top-level member two
//! times, as it skips a committed conflict marker. A committed line of
//! another record shape, or with the removed `retired` member, is a version.
//! These tests have no Rule binding: no active Rule states the result of
//! these committed lines.

use crate::discussion_support;
use discussion_support::*;
use provenance_core::review::{RecordVersion, ReviewHistoryQuery};
use provenance_core::{NodeType, StableId};
use provenance_store::{
    layout::ProvenanceLayout, operations::read_policy::ReadPolicy, review::read_history,
};
use serde_json::json;

/// The history of the Requirement `record` in the fixture at `temp`.
async fn history(temp: &tempfile::TempDir, record: &str) -> Vec<RecordVersion> {
    let root = camino::Utf8Path::from_path(temp.path()).unwrap();
    let query = ReviewHistoryQuery {
        record_kind: NodeType::Requirement,
        record_id: StableId::new(record).unwrap(),
        limit: 50,
        cursor: None,
    };
    read_history(&repository(root), ReadPolicy::default(), query)
        .await
        .unwrap()
        .result
        .entries
}

/// The layout of the fixture at `temp`.
fn layout(temp: &tempfile::TempDir) -> ProvenanceLayout {
    ProvenanceLayout::new(camino::Utf8Path::from_path(temp.path()).unwrap())
}

/// The Requirement file of the fixture at `temp`.
fn requirements(temp: &tempfile::TempDir) -> camino::Utf8PathBuf {
    provenance_store::shards::path_for(&layout(temp), &scope(), NodeType::Requirement)
}

/// Commits the file at `path` with `line` changed by `change`, then commits
/// the file as it was. Returns the commit that holds the changed line.
fn commit_changed_line(
    temp: &tempfile::TempDir,
    path: &camino::Utf8Path,
    line: &str,
    change: impl Fn(&str) -> String,
) -> String {
    let text = std::fs::read_to_string(path).unwrap();
    let changed = text
        .lines()
        .map(|each| {
            if each.contains(line) {
                change(each)
            } else {
                each.to_owned()
            }
        })
        .fold(String::new(), |mut changed, each| {
            changed.push_str(&each);
            changed.push('\n');
            changed
        });
    assert_ne!(changed, text, "no line of {path} holds {line}");
    std::fs::write(path, changed).unwrap();
    let commit = commit_state(temp, "Commit the changed line");
    std::fs::write(path, &text).unwrap();
    commit_state(temp, "Restore the line");
    commit
}

#[tokio::test]
async fn history_skips_a_committed_line_whose_last_id_names_the_record() {
    let (temp, _store) = fixture();
    let created = commit_state(&temp, "Create");
    let path = requirements(&temp);
    commit_changed_line(&temp, &path, r#""id":"req_a""#, |line| {
        line.replacen('{', r#"{"id":"req_shadow","#, 1).replace(
            "The system stores records.",
            "The shadow line holds this text.",
        )
    });

    let versions = history(&temp, "req_a").await;
    let ids = versions.iter().map(|v| v.id.as_str()).collect::<Vec<_>>();
    assert_eq!(ids, [created.as_str()]);
}

#[tokio::test]
async fn history_skips_a_committed_line_whose_first_id_names_the_record() {
    let (temp, _store) = fixture();
    commit_state(&temp, "Create");
    let path = requirements(&temp);
    commit_changed_line(&temp, &path, r#""id":"req_a""#, |line| {
        line.replace(r#""id":"req_a""#, r#""id":"req_a","id":"req_shadow""#)
    });

    assert_eq!(history(&temp, "req_shadow").await, []);
}

/// The history of `req_a` after a save from a Discussion, committed with
/// its Discussion line, and that commit. With `repeat`, the committed line
/// holds the member `actor` two times.
async fn outcome_history(repeat: bool) -> (Vec<RecordVersion>, String) {
    let (temp, store) = fixture();
    commit_state(&temp, "Create");
    let discussion = start(&store, "Describe the record.");
    let origin = provenance_core::threads::DiscussionOrigin {
        discussion_id: discussion.discussion_id.clone(),
        thread_id: discussion.thread_id.clone(),
        message_id: discussion.root_message_id.clone(),
    };
    store
        .save_requirement_from_discussion(
            save(&store, json!({"description":"Described."})),
            &origin,
        )
        .unwrap();
    let outcome = if repeat {
        let path = provenance_store::shards::discussions_path(&layout(&temp), &scope());
        let line = format!(r#""discussion_id":"{}""#, discussion.discussion_id.as_str());
        commit_changed_line(&temp, &path, &line, |line| {
            line.replacen('{', r#"{"actor":"ben","#, 1)
        })
    } else {
        commit_state(&temp, "Commit the outcome")
    };
    (history(&temp, "req_a").await, outcome)
}

#[tokio::test]
async fn history_skips_a_committed_discussion_line_that_holds_a_member_two_times() {
    let (versions, outcome) = outcome_history(false).await;
    assert_eq!(versions[1].id.as_str(), outcome);
    assert!(versions[1].origin.is_some());

    let (versions, outcome) = outcome_history(true).await;
    assert_eq!(versions.len(), 2);
    assert_eq!(versions[1].id.as_str(), outcome);
    assert_eq!(versions[1].origin, None);
}

#[tokio::test]
async fn history_shows_a_committed_version_of_another_record_shape() {
    let (temp, _store) = fixture();
    commit_state(&temp, "Create");
    let path = requirements(&temp);
    let changed = commit_changed_line(&temp, &path, r#""id":"req_a""#, |line| {
        line.replace(r#""statement":"#, r#""text":"#)
    });

    let versions = history(&temp, "req_a").await;
    let ids = versions.iter().map(|v| v.id.as_str()).collect::<Vec<_>>();
    assert!(ids.contains(&changed.as_str()), "{ids:?}");
}

#[tokio::test]
async fn history_shows_a_committed_version_that_holds_retired() {
    let (temp, _store) = fixture();
    commit_state(&temp, "Create");
    let path = requirements(&temp);
    let changed = commit_changed_line(&temp, &path, r#""id":"req_a""#, |line| {
        line.replacen('{', r#"{"retired":false,"#, 1)
    });

    let versions = history(&temp, "req_a").await;
    let ids = versions.iter().map(|v| v.id.as_str()).collect::<Vec<_>>();
    assert!(ids.contains(&changed.as_str()), "{ids:?}");
}
