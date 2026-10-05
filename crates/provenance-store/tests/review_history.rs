#[allow(dead_code)]
mod review_support;
use provenance_core::review::{ReviewHistoryQuery, SaveOutcome};
use provenance_macros::verifies;
use provenance_store::{operations::read_policy::ReadPolicy, review::read_history};
use review_support::*;
use serde_json::json;

#[tokio::test]
#[verifies("rule_record_history_reads_git", examples)]
async fn history_lists_commits_and_working_version() {
    let (temp, store) = fixture();
    let created = commit_state(&temp, "Create the Requirement");
    store
        .save_requirement(save(&store, "edit_b", json!({"description":"B"})))
        .unwrap();
    let described = commit_state(&temp, "Describe the Requirement");
    store
        .save_requirement(save(&store, "edit_c", json!({"description":"C"})))
        .unwrap();
    let root = camino::Utf8Path::from_path(temp.path()).unwrap();
    let query = ReviewHistoryQuery {
        record_kind: provenance_core::NodeType::Requirement,
        record_id: id(),
        limit: 10,
        cursor: None,
    };
    let versions = read_history(root, &scope(), ReadPolicy::default(), query)
        .await
        .unwrap()
        .result
        .entries;
    let ids = versions.iter().map(|v| v.id.as_str()).collect::<Vec<_>>();
    assert_eq!(ids, [created.as_str(), described.as_str(), "working"]);
    assert_eq!(versions[0].outcome, SaveOutcome::Created);
    assert_eq!(versions[0].before, None);
    assert_eq!(versions[0].author.as_deref(), Some("Reviewer"));
    assert_eq!(versions[1].commit.as_deref(), Some(described.as_str()));
    assert_eq!(versions[1].before.as_ref(), Some(&versions[0].id));
    assert_eq!(versions[1].changed_fields, ["description"]);
    assert_eq!(versions[2].commit, None);
    assert_eq!(versions[2].before.as_ref(), Some(&versions[1].id));
    assert_eq!(versions[2].outcome, SaveOutcome::Changed);
    assert_eq!(versions[2].changed_fields, ["description"]);
}
