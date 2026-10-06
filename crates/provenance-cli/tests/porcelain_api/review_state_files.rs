use super::review_cycle::{decision, describe, init_with_reviewer, write};
use super::review_history::{commit, git};
use super::*;

/// Flow: from a committed initial state, create, edit, withdraw, submit, decide with
/// feedback and reply; Git then reports changes to record shard files only.
#[test]
#[verifies("rule_review_writes_change_only_record_files", examples)]
fn review_writes_change_only_record_shard_files() {
    let (_directory, repo) = init_with_reviewer();
    // Init also creates skills, instructions, and ignore settings.
    git(&repo, &["add", "."]);
    commit(&repo);
    assert_eq!(
        git(&repo, &["status", "--porcelain", "--untracked-files=all"]),
        ""
    );
    write(
        &repo,
        "post",
        "requirements",
        &json!({"actor":"agent", "id":"req_files", "statement":"The file statement applies.",
            "status":"active", "depends_on":[], "supersedes":[]}),
        None,
    );

    let edited = describe(&repo, "req_files", "The files are graph records.");
    let automatic = edited["data"]["decision"]["pending"]["proposal_id"]
        .as_str()
        .unwrap()
        .to_owned();
    write(
        &repo,
        "post",
        &format!("requirements/req_files/submissions/{automatic}/withdraw"),
        &json!({"actor":"agent", "declared_by":null, "reason":"Submit with a summary."}),
        None,
    );
    let submitted = write(
        &repo,
        "post",
        "requirements/req_files/submit",
        &json!({"actor":"agent", "declared_by":null, "title":"Review the files",
            "summary":"Review the file statement.", "confidence":null, "source_ids":[],
            "evidence_references":[], "builds_on":[], "expected_revision":null, "revises":null}),
        None,
    );
    let proposal = submitted["data"]["proposal_id"]
        .as_str()
        .unwrap()
        .to_owned();
    write(
        &repo,
        "post",
        &format!("requirements/req_files/submissions/{proposal}/decide"),
        &decision("rejected", "req_files", Some("Name the record files.")),
        None,
    );
    let discussions = json(&["api", "requirements/req_files/discussions", "--repo", &repo]);
    let discussion = discussions["data"]["items"][0]["discussion"]["discussion_id"]
        .as_str()
        .unwrap()
        .to_owned();
    write(
        &repo,
        "post",
        &format!("requirements/req_files/discussions/{discussion}/messages"),
        &json!({"actor":"agent", "declared_by":null, "role":"assistant",
            "body":"The record files are JSONL files."}),
        Some("\"1\""),
    );

    let status = git(&repo, &["status", "--porcelain", "--untracked-files=all"]);
    let changed = status.lines().collect::<Vec<_>>();
    assert_ne!(changed, [] as [&str; 0]);
    for path in &changed {
        // The shared Git helper removes leading spaces from the first line.
        let path = path.trim_start().split_once(' ').unwrap().1.trim_start();
        let path = camino::Utf8Path::new(path);
        assert!(
            path.starts_with(".provenance/state/scopes/default")
                && path.extension() == Some("jsonl"),
            "{path} is not a graph record file"
        );
    }
}
