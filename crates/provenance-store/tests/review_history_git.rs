//! Record history over a merged Git history and over a shard version that
//! does not parse.
#[allow(dead_code)]
mod review_support;
use provenance_core::review::{RecordVersion, ReviewHistoryQuery};
use provenance_core::{NodeType, StableId};
use provenance_macros::verifies;
use provenance_store::{
    layout::ProvenanceLayout, operations::read_policy::ReadPolicy, review::read_history,
    state_store::StateStore,
};
use review_support::*;
use serde_json::json;

/// Runs one Git command as `author` in the fixture repository.
fn git_as(temp: &tempfile::TempDir, author: &str, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .args([
            "-c",
            &format!("user.name={author}"),
            "-c",
            &format!("user.email={author}@example.com"),
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(temp.path())
        .output()
        .unwrap();
    assert!(output.status.success(), "git {args:?} failed: {output:?}");
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

/// Commits the saved graph state as `author` and returns the commit id.
fn commit_as(temp: &tempfile::TempDir, author: &str, message: &str) -> String {
    git_as(temp, author, &["add", ".provenance/state"]);
    git_as(temp, author, &["commit", "-q", "-m", message]);
    git_as(temp, author, &["rev-parse", "HEAD"])
}

fn create(store: &StateStore, id: &str) {
    store
        .create_review_requirement(
            serde_json::from_value(json!({
                "actor":"ben","origin":null,
                "create":{"scope_id":"default","id":id,"statement":"The system stores records.",
                    "status":"discovery","depends_on":[],"supersedes":[]}
            }))
            .unwrap(),
        )
        .unwrap();
}

fn save_record(store: &StateStore, id: &str, fields: &serde_json::Value) {
    let record = StableId::new(id).unwrap();
    let mut update = json!({"scope_id":"default", "id":id});
    update
        .as_object_mut()
        .unwrap()
        .extend(fields.as_object().unwrap().clone());
    store
        .save_requirement(
            serde_json::from_value(json!({
                "actor":"ben",
                "expected_etag":store.requirement_edit_state(&scope(), &record).unwrap().etag,
                "update":update,"relationships":null
            }))
            .unwrap(),
        )
        .unwrap();
}

async fn versions(temp: &tempfile::TempDir) -> Vec<RecordVersion> {
    let root = camino::Utf8Path::from_path(temp.path()).unwrap();
    let query = ReviewHistoryQuery {
        record_kind: NodeType::Requirement,
        record_id: id(),
        limit: 50,
        cursor: None,
    };
    read_history(root, &scope(), ReadPolicy::default(), query)
        .await
        .unwrap()
        .result
        .entries
}

#[tokio::test]
#[verifies("rule_record_history_reads_git", examples)]
async fn merged_history_lists_only_the_commits_that_changed_the_record() {
    let (temp, store) = fixture();
    // Other records keep the two edits apart in the shard, so Git merges the file cleanly.
    for id in [
        "req_b", "req_c", "req_d", "req_e", "req_f", "req_g", "req_h",
    ] {
        create(&store, id);
    }
    let created = commit_state(&temp, "Create");
    git(&temp, &["checkout", "-q", "-B", "main"]);
    git(&temp, &["checkout", "-q", "-b", "feature"]);
    store
        .save_requirement(save(
            &store,
            json!({"statement":"The system stores records for one year."}),
        ))
        .unwrap();
    let alice = commit_as(&temp, "alice", "alice changes req_a");
    git(&temp, &["checkout", "-q", "main"]);
    save_record(
        &store,
        "req_h",
        &json!({"description":"Bob describes req_h."}),
    );
    commit_as(&temp, "bob", "bob describes req_h");
    git_as(&temp, "merger", &["merge", "-q", "--no-edit", "feature"]);

    let versions = versions(&temp).await;
    let ids = versions.iter().map(|v| v.id.as_str()).collect::<Vec<_>>();
    assert_eq!(ids, [created.as_str(), alice.as_str()]);
    assert_eq!(versions[1].author.as_deref(), Some("alice"));
    assert_eq!(versions[1].before.as_ref(), Some(&versions[0].id));
    assert_eq!(versions[1].changed_fields, ["statement"]);
}

#[tokio::test]
#[verifies("rule_record_history_reads_git", examples)]
async fn history_skips_a_shard_version_that_does_not_parse() {
    let (temp, store) = fixture();
    let created = commit_state(&temp, "Create");
    let layout = ProvenanceLayout::new(camino::Utf8Path::from_path(temp.path()).unwrap());
    let path = provenance_store::shards::path_for(&layout, &scope(), NodeType::Requirement);
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, format!("<<<<<<< HEAD\n{text}")).unwrap();
    commit_state(&temp, "Commit a conflict marker");
    std::fs::write(&path, &text).unwrap();
    commit_state(&temp, "Remove the conflict marker");
    store
        .save_requirement(save(&store, json!({"description":"Described."})))
        .unwrap();

    let versions = versions(&temp).await;
    let ids = versions.iter().map(|v| v.id.as_str()).collect::<Vec<_>>();
    assert_eq!(ids, [created.as_str(), "working"]);
    assert_eq!(versions[1].changed_fields, ["description"]);
}
